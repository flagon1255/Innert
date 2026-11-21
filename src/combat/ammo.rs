//! Special ammo in flight, each kind as weapons::ammo has it look: its own model, the trail it
//! leaves and the puff where it strikes. A plain arrow, bolt or ball flies as it always did.

use super::*;
use crate::weapons::{AMMO_MODELS, AMMO_RES, AMMO_SHOT_RES, AMMO_STYLES, Puff, Trail};

/// Trail sparks a second off an arrow or bolt of special ammo, and off each ball of it, fewer as a
/// spread of them flies at once
pub(super) const AMMO_TRAIL: f64 = 60.0;
pub(super) const AMMO_BALL_TRAIL: f64 = 40.0;
/// How big a ball of special ammo is drawn over its tracer, next to as big as it is
const AMMO_BALL_SIZE: f64 = 4.0;

impl Combat {
    /// One spark of special ammo `k`'s trail at `at`, flying at `velocity`
    pub(super) fn ammo_trail(&mut self, k: u8, at: Vector3<f64>, velocity: Vector3<f64>) {
        let vpm = self.vpm;
        let style = &AMMO_STYLES[k as usize];
        let look = Element::look(style.glow);
        let way = velocity.try_normalize(1e-9).unwrap_or_else(Vector3::x);
        let chance = random(&mut self.rng);
        match style.trail {
            Trail::Blood => self.drop(at, way * 0.5 * vpm, DROP_BLOOD, 0.6),
            Trail::Embers if chance < 0.3 => self.drop(at, Vector3::new(0.0, 0.0, 0.8 * vpm), DROP_EMBER, 0.6),
            Trail::Embers => self.mote(at, Some(Element::Fire), 1.4),
            Trail::Frost if chance < 0.2 => self.drop(at, Vector3::zeros(), DROP_ICE, 0.7),
            Trail::Frost => self.mote(at, Some(Element::Frost), 1.6),
            Trail::Venom if chance < 0.55 => self.drop(at, Vector3::zeros(), DROP_VENOM, 0.7),
            Trail::Venom => self.mote(at, Some(Element::Poison), 1.2),
            Trail::Smoke if chance < 0.35 => self.fine(at, Vector3::zeros(), Element::look(Some(Element::Fire)), 0.25, -0.1),
            Trail::Smoke => {
                let drift = self.any_direction() * 0.3 * vpm;
                self.sparks.push(Spark { position: at, velocity: drift, look: Element::look(None), life: 0.55, lifetime: 0.55, gravity: -0.04, slot: SLOT_SMOKE });
            }
            Trail::Tracer => self.sparks.push(Spark { position: at, velocity: way * 1.5 * vpm, look, life: 0.18, lifetime: 0.18, gravity: 0.0, slot: SLOT_SPARK }),
            Trail::Glint => {
                let side = self.any_direction() * 1.2 * vpm;
                self.sparks.push(Spark { position: at, velocity: side, look: Element::look(Some(Element::Holy)), life: 0.22, lifetime: 0.22, gravity: 0.3, slot: SLOT_SPARK });
            }
            Trail::Dust => self.drop(at, Vector3::zeros(), DROP_MUD, 0.5),
            Trail::Crackle => self.mote(at, Some(Element::Storm), 1.6),
            Trail::Void => self.mote(at, Some(Element::Void), 1.6),
            Trail::Holy if chance < 0.25 => self.drop(at, Vector3::zeros(), DROP_GOLD, 0.6),
            Trail::Holy => self.mote(at, Some(Element::Holy), 1.6),
            Trail::Miasma => {
                let drift = self.any_direction() * 0.25 * vpm;
                self.sparks.push(Spark { position: at, velocity: drift, look: Element::look(Some(Element::Poison)), life: 0.7, lifetime: 0.7, gravity: 0.05, slot: SLOT_SMOKE });
            }
            Trail::Arcane => self.mote(at, Some(Element::Arcane), 1.6),
            Trail::Brimstone if chance < 0.5 => self.drop(at, Vector3::zeros(), DROP_EMBER, 0.8),
            Trail::Brimstone => {
                let drift = self.any_direction() * 0.3 * vpm;
                self.sparks.push(Spark { position: at, velocity: drift, look: Element::look(Some(Element::Fire)), life: 0.5, lifetime: 0.5, gravity: -0.05, slot: SLOT_SMOKE });
            }
            Trail::Grit => {
                let v = self.any_direction() * 0.4 * vpm;
                self.drop(at, v, DROP_ASH, 0.6);
            }
            Trail::Fibres => self.fine(at, -way * 0.6 * vpm, Element::look(None), 0.35, 0.1),
            Trail::Sparks => {
                let out = (self.any_direction() - way) * 2.5 * vpm;
                self.sparks.push(Spark { position: at, velocity: out, look, life: 0.2, lifetime: 0.2, gravity: 0.6, slot: SLOT_SPARK });
            }
            Trail::Twin => {
                let across = way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x) * 0.12 * vpm;
                for side in [-1.0, 1.0] {
                    self.fine(at + across * side, Vector3::zeros(), Element::look(None), 0.3, 0.0);
                }
            }
        }
        self.cap_sparks();
    }

    /// Special ammo `k` striking at `at`: a puff as fits what it is
    pub(super) fn ammo_puff(&mut self, k: u8, at: Vector3<f64>) {
        let vpm = self.vpm;
        let style = &AMMO_STYLES[k as usize];
        let look = Element::look(style.glow);
        let up = Vector3::z();
        match style.puff {
            Puff::Blood => {
                for _ in 0..6 {
                    let v = (self.any_direction() + up) * 2.2 * vpm;
                    self.drop(at, v, DROP_BLOOD, 0.7);
                }
            }
            Puff::Fire => self.sparks(at, 8, 3.0, Element::look(Some(Element::Fire)), -0.2, 0.5),
            Puff::Frost => {
                self.sparks(at, 6, 2.0, Element::look(Some(Element::Frost)), 0.1, 0.6);
                for _ in 0..3 {
                    let v = (self.any_direction() + up) * 1.8 * vpm;
                    self.drop(at, v, DROP_ICE, 0.8);
                }
            }
            Puff::Venom => {
                for _ in 0..6 {
                    let v = (self.any_direction() + up * 0.6) * 2.0 * vpm;
                    self.drop(at, v, DROP_VENOM, 0.8);
                }
            }
            Puff::Powder | Puff::Dust | Puff::Miasma => {
                let (smoke, n) = match style.puff {
                    Puff::Powder => (Element::look(Some(Element::Fire)), 5),
                    Puff::Miasma => (Element::look(Some(Element::Poison)), 6),
                    _ => (Element::look(None), 4),
                };
                for _ in 0..n {
                    let v = (self.any_direction() + up * 0.8) * 0.9 * vpm;
                    self.sparks.push(Spark { position: at, velocity: v, look: smoke, life: 0.9, lifetime: 0.9, gravity: -0.03, slot: SLOT_SMOKE });
                }
                if style.puff == Puff::Dust {
                    for _ in 0..4 {
                        let v = (self.any_direction() + up) * 2.0 * vpm;
                        self.drop(at, v, DROP_MUD, 0.7);
                    }
                }
            }
            Puff::Sparks => self.sparks(at, 10, 5.0, Element::look(None), 0.8, 0.3),
            Puff::Storm => self.sparks(at, 10, 4.0, Element::look(Some(Element::Storm)), 0.0, 0.25),
            Puff::Void => self.sparks(at, 10, -2.5, Element::look(Some(Element::Void)), 0.0, 0.4),
            Puff::Holy => {
                self.sparks(at, 8, 2.5, Element::look(Some(Element::Holy)), -0.2, 0.6);
                self.drop(at, up * 2.0 * vpm, DROP_GOLD, 0.8);
            }
            Puff::Arcane => self.sparks(at, 8, 2.5, Element::look(Some(Element::Arcane)), -0.1, 0.5),
            Puff::Grit => {
                for _ in 0..8 {
                    let v = (self.any_direction() + up * 0.5) * 3.0 * vpm;
                    self.drop(at, v, DROP_ASH, 0.7);
                }
            }
            Puff::Shards => self.sparks(at, 8, 4.5, look, 1.0, 0.35),
            Puff::Net => {
                for _ in 0..6 {
                    let v = (self.any_direction() + up) * 1.5 * vpm;
                    self.fine(at, v, Element::look(None), 0.5, 0.5);
                }
            }
        }
    }

    /// What to draw for special ammo `k` flying as `p`: an arrow's or a bolt's own model in place
    /// of a plain one, see projectile_instance(), or a ball's drawn over its tracer, see
    /// flying_instances()
    pub(super) fn ammo_instance(&self, k: u8, p: &Projectile, view: &View, rotation: UnitQuaternion<f64>) -> AnimalInstance {
        let vpm = view.vpm;
        let (model, shape) = self.model_at(AMMO_MODELS + k as usize);
        match p.shot {
            Shot::Arrow => place(model, shape, AMMO_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), p.position, rotation, vpm, 1.0, MOVING),
            Shot::Quarrel => place(model, shape, AMMO_RES, Vector3::new(QUARREL_HEAD, 0.0, 0.0), p.position, rotation, vpm, p.size, MOVING),
            // A ball tumbling, drawn bigger the further off it is, never so big up close it fills
            // the view
            _ => {
                let d = (p.position - view.eye).norm() / vpm;
                let spin = rotation * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.time * 11.0);
                let scale = AMMO_BALL_SIZE * (d / 3.0).clamp(0.3, 1.0) * (d / 15.0).max(1.0).sqrt();
                place(model, shape, AMMO_SHOT_RES, Vector3::zeros(), p.position, spin, vpm, scale, MOVING)
            }
        }
    }

    /// Every kind of special ammo in `library` hung in rows in front of `eye`, looking along
    /// `forward`, each flying slowly across the view as it would, made as `profile`'s in its own
    /// element, for a contact sheet of them, see `--script ammo_sheet`. Row by row, seven to a
    /// row, in the order of AMMO_STYLES.
    pub fn ammo_sheet(&mut self, library: &crate::weapons::Library, profile: &Rc<Profile>, eye: Vector3<f64>, forward: Vector3<f64>) {
        let vpm = self.vpm;
        let forward = Vector3::new(forward.x, forward.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::y);
        let right = forward.cross(&Vector3::z());
        for (k, style) in AMMO_STYLES.iter().enumerate() {
            let Some(def) = library.ammo.iter().find(|a| a.id == style.id) else {
                continue;
            };
            let (row, column) = (k / 7, k % 7);
            let at = eye + (forward * 3.6 + right * (column as f64 - 3.0) * 0.8 + Vector3::z() * (0.9 - row as f64 * 0.42)) * vpm;
            let profile = Rc::new(Profile { element: def.element, ..(**profile).clone() });
            let missile = Missile { shot: def.shot(), head: def.head.unwrap_or(0), power: true, size: 1.0 };
            self.launch(&profile, Some(missile), Form::Bolt, at, right * 0.8 * vpm, 0.0, 0.0);
            if let Some(p) = self.projectiles.last_mut() {
                p.ammo = Some(k as u8);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapons::Library;

    /// Which model `instance` draws: its info's first word, see AnimalInstance::new()
    fn model_of(instance: AnimalInstance) -> u32 {
        // SAFETY: AnimalInstance is repr(C), four arrays of four 32-bit words
        let words: [u32; 16] = unsafe { std::mem::transmute(instance) };
        words[12]
    }

    /// The rule for special ammo: every kind in assets\weapons.json has a look of its own, a model
    /// unlike every other's, and among what shoots the same, arrows, bolts or shot, a glow and a
    /// trail no other has, glowing its element's colors if it has one; all their models together
    /// well under 2 MB
    #[test]
    fn every_ammo_kind_looks_its_own() {
        let library = Library::load();
        for def in &library.ammo {
            assert!(crate::weapons::ammo_style(&def.id).is_some(), "{} has no look of its own", def.id);
        }
        let models: Vec<_> = AMMO_STYLES.iter().map(|s| (s.model)()).collect();
        let mut bytes = 0;
        for (k, style) in AMMO_STYLES.iter().enumerate() {
            let def = library.ammo.iter().find(|a| a.id == style.id).unwrap_or_else(|| panic!("{} isn't any ammo", style.id));
            let (model, _) = &models[k];
            let filled = model.voxels.iter().filter(|&&v| v != 0).count();
            assert!(filled > 40, "{}'s model is all but empty: {filled} of {} voxels, {:?}", style.id, model.voxels.len(), model.size);
            bytes += model.voxels.len();
            for (j, other) in AMMO_STYLES.iter().enumerate().take(k) {
                assert!(models[j].0.voxels != model.voxels || models[j].0.size != model.size, "{} and {} have the same model", style.id, other.id);
                let same_shot = library.ammo.iter().find(|a| a.id == other.id).is_some_and(|a| a.fits == def.fits);
                assert!(!(same_shot && other.glow == style.glow && other.trail == style.trail), "{} and {} glow and trail alike", style.id, other.id);
            }
            if def.element.is_some() {
                assert_eq!(style.glow, def.element, "{} glows other than its element", style.id);
            }
        }
        assert!(bytes < 2 << 20, "the ammo's models take {bytes} bytes");
        println!("{} kinds of ammo, their models {} kB", AMMO_STYLES.len(), bytes / 1024);
    }

    /// Shot with each kind of special ammo loaded, from a weapon that shoots it, out over open
    /// ground: what flies is drawn as that kind's own model, leaves its trail, is never too small
    /// to see however far it gets, and puffs where it lands
    #[test]
    fn every_ammo_kind_flies_as_it_looks() {
        let library = Library::load();
        let vpm = 8.0;
        let size = [1536u32, 1536, 64];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 4].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, crate::worldgen::material::DIRT);
        world.voxels_per_meter = 8;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let feet = Vector3::new(96.0, 20.0, 4.0) * vpm;
        let view = View { eye: feet + Vector3::new(0.0, 0.0, 1.6 * vpm), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.03), tan: [1.2445, 0.7002], vpm, speed: 0.0, stride: 0.0 };
        let mut broken = Vec::new();
        for (k, style) in AMMO_STYLES.iter().enumerate() {
            let a = library.ammo.iter().position(|a| a.id == style.id).expect("the ammo");
            let def = &library.ammo[a];
            let family = library.family(match def.fits.as_str() {
                "arrow" => "bow",
                "bolt" => "crossbow",
                _ => "musket",
            });
            let weapon = library.generate(7, family, 0.0);
            let loaded = Loaded { ammo: a, profile: Rc::new(library.loaded(&weapon, a)), head: def.head, burst: def.burst, style: Some(k) };
            let mut animals = Vec::new();
            let mut combat = Combat::new(1, 0);
            combat.vpm = vpm;
            {
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet), water: &water };
                if def.fits == "arrow" {
                    combat.loose(&mut arena, &weapon, Some(&loaded), 1.0);
                } else {
                    combat.fire(&mut arena, 0, &weapon, Some(&loaded));
                }
            }
            let ours = combat.first_model + (AMMO_MODELS + k) as u32;
            if combat.projectiles.is_empty() || combat.projectiles.iter().any(|p| p.ammo != Some(k as u8)) {
                broken.push(format!("{}: what's shot isn't marked as it", style.id));
                continue;
            }
            let (mut drawn, mut sparks, mut furthest, mut puffed) = (false, 0, 0.0f64, false);
            for _ in 0..300 {
                let before = combat.sparks.len();
                let flying = combat.projectiles.len();
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet), water: &water };
                combat.update(&mut arena, None, None, Controls::default(), 1.0 / 60.0);
                sparks += combat.sparks.len().saturating_sub(before);
                puffed |= combat.projectiles.len() < flying && combat.sparks.len() > before;
                let instances = combat.flying_instances(&view);
                drawn |= instances.iter().any(|(_, i)| model_of(*i) == ours);
                if let Some(p) = combat.projectiles.first() {
                    let d = (p.position - view.eye).norm();
                    furthest = furthest.max(d / vpm);
                    let seen = instances.iter().any(|(_, i)| {
                        let (at, reach) = i.sphere();
                        (at - p.position).norm() < 1.5 * vpm && reach / d > 0.002
                    });
                    // Not past where everything's faded into the sky, where nothing's drawn
                    if d > 6.0 * vpm && d < combat.seen && !seen {
                        broken.push(format!("{}: too small to see {:.0} m off", style.id, d / vpm));
                        break;
                    }
                }
                if combat.projectiles.is_empty() {
                    break;
                }
            }
            if !drawn {
                broken.push(format!("{}: never drawn as its own model", style.id));
            }
            if sparks < 10 {
                broken.push(format!("{}: only {sparks} sparks of trail", style.id));
            }
            if !puffed {
                broken.push(format!("{}: never puffed where it landed, {furthest:.0} m off", style.id));
            }
        }
        assert!(broken.is_empty(), "{}", broken.join("\n"));
    }
}
