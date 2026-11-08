//! Where the great serpents keep their lairs, in whichever world the player's in: in Realms each
//! realm's own great serpent, see serpents::realm_serpent(); in Free Roam lake serpents here and
//! there in the deep water near a shore, or frost serpents under the ice where it's cold, one in a
//! square of the landscape at most, see FREE_ROAM_CELL.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::animals::Look;
use crate::serpents;

use super::*;

/// How big a square of the landscape has at most one great serpent's lair in Free Roam, in meters,
/// and the share of them that do
const FREE_ROAM_CELL: f64 = 700.0;
const FREE_ROAM_SHARE: u32 = 55;

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}

/// The great serpent's lair in square `cell` of Free Roam's landscape `settings`, if it has one:
/// who's there and where, in meters. Worked out once for each square, as looking for water takes a
/// while.
fn free_roam_lair(settings: &TerrainSettings, cell: [i64; 2]) -> Option<(Look, [f64; 2])> {
    static FOUND: Mutex<Option<HashMap<(u32, i64, i64), Option<(Look, [f64; 2])>>>> = Mutex::new(None);
    let key = (settings.seed, cell[0], cell[1]);
    if let Some(found) = FOUND.lock().ok().and_then(|f| f.as_ref().and_then(|f| f.get(&key).copied())) {
        return found;
    }
    let roll = hash(settings.seed ^ hash(cell[0] as u32 ^ hash(cell[1] as u32 ^ 0x5e4f)));
    let found = (roll % 100 < FREE_ROAM_SHARE)
        .then(|| {
            let (fx, fy) = ((roll >> 8 & 0xff) as f64 / 255.0, (roll >> 16 & 0xff) as f64 / 255.0);
            let near = [(cell[0] as f64 + 0.2 + 0.6 * fx) * FREE_ROAM_CELL, (cell[1] as f64 + 0.2 + 0.6 * fy) * FREE_ROAM_CELL];
            let spot = serpents::water_near(settings, near, 180.0, 1.5, 24.0)?;
            let cold = worldgen::climate(settings, spot[0] as f32, spot[1] as f32).snow > 0.5;
            Some((if cold { Look::Shatterfang } else { Look::Meremaw }, spot))
        })
        .flatten();
    if let Ok(mut f) = FOUND.lock() {
        f.get_or_insert_with(HashMap::new).insert(key, found);
    }
    found
}

/// What a great serpent's lair at `spot`, in meters, is called, to remember it's beaten
fn key_of(spot: [f64; 2]) -> [i64; 3] {
    [(spot[0] * 10.0).round() as i64, (spot[1] * 10.0).round() as i64, serpents::LAIR_KEY]
}

/// What's kept in the save of the great serpents' lairs, see Save::section(): the lairs found in
/// Free Roam, to be marked on the map, by who's there and where, in meters
#[derive(serde::Serialize, serde::Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(default)]
pub struct SerpentsSave {
    pub found: Vec<(String, [f64; 2])>,
}

/// Its name in the save, see save::Save::sections
const SECTION: &str = "serpents";

/// How near a great serpent's lair the player has to come in Free Roam to find it, in meters
const FIND_M: f64 = 120.0;

impl App {
    /// What's kept in the save of the great serpents' lairs
    fn serpents_save(&self) -> SerpentsSave {
        self.session.sections.get(SECTION).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
    }

    fn keep_serpents_save(&mut self, kept: &SerpentsSave) {
        if let Ok(value) = serde_json::to_value(kept) {
            self.session.sections.insert(SECTION.to_owned(), value);
        }
    }

    /// The great serpents' lairs to mark on the map: the realm's own in Realms, and those found in
    /// Free Roam, with who's there and whether they're beaten
    pub(super) fn serpent_marks(&self) -> Vec<([f64; 2], &'static str, bool)> {
        match (&self.session.places, self.session.mode) {
            (Some(places), Some(save::Mode::Realms)) => places.serpent.zip(places.serpent_key()).map(|((look, spot), key)| (spot, look.name(), self.game.is_defeated(&key))).into_iter().collect(),
            (_, Some(save::Mode::FreeRoam)) => self
                .serpents_save()
                .found
                .iter()
                .filter_map(|(name, spot)| {
                    let look = Look::ALL.into_iter().find(|l| l.is_great_serpent() && l.name() == name)?;
                    Some((*spot, look.name(), self.game.is_defeated(&key_of(*spot))))
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Now and then in Free Roam, the great serpents' lairs the player's come near enough to find,
    /// kept to be marked on the map from then on
    pub(super) fn find_serpent_lairs(&mut self) {
        if self.session.mode != Some(save::Mode::FreeRoam) {
            return;
        }
        let vpm = self.world.voxels_per_meter as f64;
        let Some(feet) = self.player.as_ref().map(|p| p.position) else {
            return;
        };
        let mut kept = self.serpents_save();
        let mut newly = Vec::new();
        for lair in self.serpent_sites() {
            let spot = [(lair.position.x + self.world.origin[0] as f64) / vpm, (lair.position.y + self.world.origin[1] as f64) / vpm];
            let found = kept.found.iter().any(|(_, at)| (at[0] - spot[0]).hypot(at[1] - spot[1]) < 1.0);
            if !found && (lair.position - feet).xy().norm() < FIND_M * vpm {
                kept.found.push((lair.look.name().to_owned(), spot));
                newly.push(lair.look);
            }
        }
        if !newly.is_empty() {
            self.keep_serpents_save(&kept);
            for look in newly {
                self.game.say(format!("You've found the lair of the {}. It's marked on your map (M).", look.name()));
            }
        }
    }

    /// The great serpents' lairs in the world where it is now, in its voxels, for adopt_features()
    pub(super) fn serpent_sites(&self) -> Vec<game::Lair> {
        let vpm = self.world.voxels_per_meter as f64;
        let [sx, sy, _] = self.world.size.map(f64::from);
        let origin = [self.world.origin[0] as f64 / vpm, self.world.origin[1] as f64 / vpm];
        let local = |p: [f64; 2]| Vector3::new(p[0] * vpm - self.world.origin[0] as f64, p[1] * vpm - self.world.origin[1] as f64, 0.0);
        let inside = |p: &Vector3<f64>| p.x >= 2.0 && p.y >= 2.0 && p.x < sx - 2.0 && p.y < sy - 2.0;
        let mut found: Vec<(Look, [f64; 2], [i64; 3])> = Vec::new();
        match (&self.session.places, self.session.mode) {
            // The realm's own
            (Some(places), Some(save::Mode::Realms)) => {
                if let (Some((look, spot)), Some(key)) = (places.serpent, places.serpent_key()) {
                    found.push((look, spot, key));
                }
            }
            // Here and there in Free Roam, in every square of the landscape the world takes in
            (_, Some(save::Mode::FreeRoam)) => {
                let settings = self.terrain_settings();
                let lo = [(origin[0] / FREE_ROAM_CELL).floor() as i64, (origin[1] / FREE_ROAM_CELL).floor() as i64];
                let hi = [((origin[0] + sx / vpm) / FREE_ROAM_CELL).floor() as i64, ((origin[1] + sy / vpm) / FREE_ROAM_CELL).floor() as i64];
                for cx in lo[0]..=hi[0] {
                    for cy in lo[1]..=hi[1] {
                        if let Some((look, spot)) = free_roam_lair(&settings, [cx, cy]) {
                            found.push((look, spot, key_of(spot)));
                        }
                    }
                }
            }
            _ => {}
        }
        found
            .into_iter()
            .filter_map(|(look, spot, key)| {
                let mut at = local(spot);
                inside(&at).then(|| {
                    at.z = game::ground_top(&self.world, at.x, at.y);
                    game::Lair { look, position: at, key, raid: None }
                })
            })
            .collect()
    }
}
