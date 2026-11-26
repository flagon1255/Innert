//! The demi-bosses down at the bottoms of ravines, see src\demis.rs: turning up in their lairs,
//! and what they leave when they're beaten, more than a monster does, less than a boss.

use super::*;
use crate::demis::{self, DEMI_HEALTH};

/// Where a demi-boss waits: on the floor of a ravine's hollow, in voxels, facing the way down into
/// it, and where in the whole landscape, to remember it's beaten
pub struct DemiLair {
    pub position: Vector3<f64>,
    pub heading: f64,
    pub key: [i64; 3],
}

/// How lucky the weapon a demi-boss leaves is, as for Library::odds(), before a realm's scaling:
/// far luckier than a monster's, not so lucky as a boss's best, see BOSS_LUCKS, and how likely it
/// is to have a special attack
const DEMI_LUCK: f64 = 3.0;
const DEMI_SPECIAL: f64 = 0.15;
/// How likely it is to leave a jewel, and how lucky that is
const DEMI_JEWEL: f64 = 0.25;
const DEMI_JEWEL_LUCK: f64 = 4.0;
/// How near a demi-boss's lair the player comes, in meters, for it to be marked on the map
const DEMI_SEEN: f64 = 45.0;
/// What the lairs come near are kept under in the save of each world, see save::WorldSave::section()
pub(super) const SEEN_SECTION: &str = "demis_seen";

impl Game {
    /// The demi-bosses' lairs in the piece of the world just built, in its voxels: one turns up in
    /// each that isn't beaten and doesn't have it already
    pub fn set_demis(&mut self, world: &World, water: WaterLevel, lairs: Vec<DemiLair>) {
        for lair in &lairs {
            let here = self.animals.iter().any(|a| a.demi.is_some() && a.boss.as_ref().is_some_and(|b| b.key == lair.key));
            if here || self.defeated.contains(&lair.key) {
                continue;
            }
            let kind = demis::kind_at(lair.key);
            let look = demis::KINDS[kind as usize].look;
            let seed = hash(lair.key[0] as u32 ^ hash(lair.key[1] as u32 ^ 0xde31));
            let Some(mut demi) = animals::spawn_monster(world, water, look, lair.position, 1.5, seed) else {
                continue;
            };
            if let Some(state) = &mut demi.boss {
                state.key = lair.key;
            }
            demi.demi = Some(kind);
            // As tough as four of its kind before its level, see Game::update()
            (demi.health, demi.max_health) = (DEMI_HEALTH, DEMI_HEALTH);
            demi.face(lair.heading);
            self.animals.push(demi);
        }
        self.demis = lairs;
    }

    /// What the player's slain is: a demi-boss, or as slain_of() says
    pub(super) fn slain(&self, animal: &Animal) -> Slain {
        if animal.demi.is_some() { Slain::Demi } else { slain_of(animal.look, animal.boss.as_ref().map(|b| b.key), &self.raid_lairs) }
    }

    /// A demi-boss beaten: beaten for good, and what it hoarded bursting out of it onto the floor
    /// round about, a weapon of about its level, gold and a potion, now and then a jewel
    pub(super) fn demi_spoils(&mut self, world: &World, animal: &Animal, kind: u8, from: (u32, Source)) {
        let vpm = world.voxels_per_meter as f64;
        let key = animal.boss.as_ref().map_or([0; 3], |b| b.key);
        self.defeated.insert(key);
        self.milestone = true;
        let sure = random(&mut self.rng) < DEMI_SPECIAL;
        let mut loot = vec![self.roll_weapon_for(animal.look, DEMI_LUCK, sure, from)];
        loot.push(Item::Gold(40 + (random(&mut self.rng) * 50.0) as u32));
        loot.push(Item::Potion);
        let best = loot.iter().filter_map(|&item| match item {
            Item::Weapon(w) => Some(self.weapons[w].grade),
            _ => None,
        }).max().unwrap_or(0);
        let middle = animal.position + Vector3::new(0.0, 0.0, animal.look.body().0 * animal.grown() * vpm);
        self.scatter(world, middle, animal.position, loot, 1.6);
        self.combat.fell(middle, animal.look.body().1 * animal.grown());
        self.shake = self.shake.max(0.5);
        self.cues.push(Cue::here(Sound::Fanfare, 0.6));
        let rarity = self.library.rarities[best].display_name.to_lowercase();
        self.say(format!("{} defeated {}! What it hoarded down here lies scattered round. The best of it's {rarity}.", self.character.name, demis::name(kind, key)));
        if random(&mut self.rng) < DEMI_JEWEL {
            let jewel = self.roll_jewel(DEMI_JEWEL_LUCK);
            self.find_jewels(vec![jewel]);
        }
    }

    /// The demi-bosses' lairs the player's come near, with `feet` where they stand, in voxels:
    /// near enough to look down into its ravine, or to have found it on the way along it
    pub(super) fn note_demis(&mut self, feet: Vector3<f64>, vpm: f64) {
        for lair in &self.demis {
            if (lair.position - feet).xy().norm() < DEMI_SEEN * vpm {
                self.demis_seen.insert(lair.key);
            }
        }
    }

    /// The demi-bosses' lairs come near, for the map: where each is in meters of the landscape,
    /// who's down there, and whether they're beaten
    pub fn demi_marks(&self) -> Vec<([f64; 2], &'static str, bool)> {
        let mut marks: Vec<([f64; 2], &'static str, bool)> = self.demis_seen.iter().map(|&key| ([key[0] as f64 / 10.0, key[1] as f64 / 10.0], demis::name(demis::kind_at(key), key), self.defeated.contains(&key))).collect();
        marks.sort_by(|a, b| a.0[0].total_cmp(&b.0[0]).then(a.0[1].total_cmp(&b.0[1])));
        marks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A demi-boss turns up in its lair, between a monster and a boss: tougher than four of its kind
    /// and levelled between them, called by its name over its own bar. Beaten, it leaves more than a
    /// monster does, and stays beaten through the save, never rising again where it was.
    #[test]
    fn a_demi_boss_is_beaten_for_good() {
        let size = [64u32, 64, 64];
        let mut occupancy = vec![0u128; 16 * 16 * 8];
        occupancy[..16 * 16 * 2].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, DIRT);
        world.voxels_per_meter = 4;
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let ground = Vector3::new(32.0, 32.0, 16.0);
        let mut game = Game::new(&world, &dry, 5, ground);
        game.animals.clear();
        let key = [4321, -876, 52];
        let lair = |key: [i64; 3]| DemiLair { position: ground, heading: 0.5, key };
        game.set_demis(&world, &dry, vec![lair(key)]);
        game.set_demis(&world, &dry, vec![lair(key)]);
        assert_eq!(game.animals.iter().filter(|a| a.demi.is_some()).count(), 1, "one, however often its lair's set");
        let player = Vector3::new(32.0, 40.0, 16.0);
        // Its level's toughness and the realm's come on top of this, as for every monster
        let demi = game.animals.iter().find(|a| a.demi.is_some()).unwrap();
        assert!((3.0..=5.0).contains(&(demi.max_health / levels::MONSTER_HEALTH)) && demi.max_health > demi.look.health() * 2.0, "{}", demi.max_health);
        assert_eq!(game.slain(demi), Slain::Demi);
        assert!(demi.grown() > 1.0 && demi.instance(4).sphere().1 > demi.look.reach() * 4.0);
        let name = demis::name(demi.demi.unwrap(), key);
        // Engaged, its name's over its bar along the bottom
        game.animals.iter_mut().filter(|a| a.demi.is_some()).for_each(|a| a.boss.as_mut().unwrap().engaged = true);
        assert_eq!(game.foe(player).map(|f| f.6), Some(name));
        // Slain, it leaves a weapon, gold and a potion, and is beaten
        let before = game.pickups.len();
        game.animals.iter_mut().filter(|a| a.demi.is_some()).for_each(|a| a.health = 0.0);
        game.reap(&world);
        assert!(game.is_defeated(&key));
        let left: Vec<Item> = game.pickups[before..].iter().map(|p| p.item).collect();
        assert!(left.iter().any(|i| matches!(i, Item::Weapon(_))) && left.iter().any(|i| matches!(i, Item::Gold(g) if *g >= 40)) && left.contains(&Item::Potion), "{left:?}");
        // Through the save into another game: it doesn't rise again, though another would
        let carried: save::CarriedSave = serde_json::from_str(&serde_json::to_string(&game.carried_save()).unwrap()).unwrap();
        let done: save::WorldSave = serde_json::from_str(&serde_json::to_string(&game.world_save()).unwrap()).unwrap();
        let mut other = Game::new(&world, &dry, 9, ground);
        other.restore(&carried, &done, game.vitals());
        other.set_demis(&world, &dry, vec![lair(key)]);
        assert!(!other.animals.iter().any(|a| a.demi.is_some()));
        other.set_demis(&world, &dry, vec![lair([1, 2, 3])]);
        assert!(other.animals.iter().any(|a| a.demi.is_some()));
        // And it's marked on the map once come near
        other.note_demis(ground, 4.0);
        assert_eq!(other.demi_marks().len(), 1);
        // Kept with the rest of the world in the save
        let done: save::WorldSave = serde_json::from_str(&serde_json::to_string(&other.world_save()).unwrap()).unwrap();
        let mut again = Game::new(&world, &dry, 9, ground);
        again.restore(&carried, &done, game.vitals());
        assert_eq!(again.demi_marks(), other.demi_marks());
    }
}