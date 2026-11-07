//! How the graphics card's memory is shared out, so the game never needs more than Windows gives
//! it. Past that, Windows moves some of it out to the computer's memory: every frame gets slower,
//! and the next piece of the world can't be sent over in time, so the world's seen to end.
//!
//! Chosen once at the start, from how much there is, see plan(): how wide the piece of the
//! landscape kept around the player is, and so how far one sees, whether what the last piece was
//! kept in is held on to for the next, and how big the light cache is. The render scale is kept
//! within what's left as the game goes on, see App::render_room() in main.rs.
//!
//! A quality preset asks for a width, see plan(): it gets as much of it as there's room for.

use std::sync::{Condvar, Mutex};
use std::time::Duration;

use crate::worldgen;

/// How much of the graphics card's memory the game may use, in bytes, once the card's picked, see
/// set_card(). Some(None) for a card that couldn't say.
static CARD: (Mutex<Option<Option<u64>>>, Condvar) = (Mutex::new(None), Condvar::new());

/// Says how much of the picked graphics card's memory the game may use, see perf::vram()
pub fn set_card(budget: Option<u64>) {
    let (card, picked) = &CARD;
    *card.lock().unwrap_or_else(|e| e.into_inner()) = Some(budget);
    picked.notify_all();
}

/// How much of the graphics card's memory the game may use, waiting for it to be picked, as the
/// world's started on another thread meanwhile. None if it can't say.
pub fn card_budget() -> Option<u64> {
    let (card, picked) = &CARD;
    let guard = card.lock().unwrap_or_else(|e| e.into_inner());
    let (guard, _) = picked.wait_timeout_while(guard, Duration::from_secs(20), |c| c.is_none()).unwrap_or_else(|e| e.into_inner());
    guard.flatten()
}

/// The widths the piece of the landscape kept around the player can be, in meters, widest first.
/// One sees half of it, less RECENTER_M in main.rs, before things fade into the sky.
pub const WIDTHS_M: &[u32] = &[224, 192, 176, 160, 144, 128];

/// Stored bricks per column of voxels: pieces 224 m wide at 16 voxels a meter had 0.06 (765,000
/// bricks) to 0.11 (1.45 million, with a fallen city under it). Planned for near the most.
const BRICKS_PER_COLUMN: f64 = 0.1;

/// About how much of the graphics card's memory a piece of the landscape `width_m` wide and
/// `height_m` tall takes at `vpm` voxels a meter: its bricks with room for holes blown in it, the
/// map of them, the grass, and the water's ground, depth, flow and surface (the last two in half
/// floats, see water.rs)
pub fn piece_bytes(width_m: u32, height_m: u32, vpm: u32) -> u64 {
    let side = (width_m * vpm) as u64;
    let columns = side * side;
    let bricks = (columns as f64 * BRICKS_PER_COLUMN) as u64 + worldgen::SPARE_BRICKS as u64;
    let grid = columns / 64 * (height_m * vpm / 8) as u64 * 4;
    bricks * worldgen::BRICK_WORDS as u64 * 4 + grid + columns * 4 + columns * (4 + 4 + 8 + 8)
}

/// Light cache entries, LIGHT_CACHE_WORDS words each in main.rs, for a card with `budget` bytes
fn light_cache_entries(budget: u64) -> u64 {
    match budget >> 20 {
        mb if mb >= 4096 => 1 << 21,
        mb if mb >= 3072 => 1 << 20,
        _ => 1 << 19,
    }
}

/// What everything but the pieces of the world takes, measured on an RTX 4050 laptop GPU: the
/// models of animals and weapons, 140 MB, the shaders, 216 MB, and about 500 MB more from the
/// first frame on, the most of it what the driver keeps for each of the GPU's threads to run
/// traverse.comp with, which a smaller card has fewer of; but not the light cache or what's
/// rendered into
const OTHERS: u64 = 800 << 20;

/// Of what there is, this much is planned for: the rest is for what comes and goes, like the
/// render scale changing, and for other programs
const PLANNED: f64 = 0.85;

/// How the graphics card's memory is shared out, see plan()
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    /// How wide the piece of the landscape kept around the player is, in meters
    pub width_m: u32,
    /// Whether what the last piece was kept in is held on to for the next to be built into,
    /// which spares a frame of 20 ms or so each time the world moves on, see Spare in main.rs.
    /// Without, two pieces are only there at once while moving on.
    pub keep_spare: bool,
    /// Entries in the light cache, see LIGHT_CACHE_WORDS in main.rs
    pub light_cache_entries: u64,
    /// What the graphics card has for the game, in bytes, if it could say
    pub budget: Option<u64>,
}

/// How the graphics card's memory is shared out, for `budget` bytes of it, None if unknown, with a
/// piece of the landscape `wanted_m` wide wanted, `height_m` tall at `vpm` voxels a meter, and
/// `render` bytes for what's rendered into at the usual render scale. The widest piece that fits
/// twice, as moving on to the next takes, beside the rest, and no wider than wanted, or the
/// narrowest there is. A spare's kept when there's room for two pieces: moving on takes that much
/// anyway, and with it there's no frame held up making room for the next. When there isn't, the
/// second's only there while moving on.
pub fn plan(budget: Option<u64>, wanted_m: u32, height_m: u32, vpm: u32, render: u64) -> Plan {
    let Some(budget) = budget else {
        return Plan { width_m: wanted_m, keep_spare: true, light_cache_entries: light_cache_entries(u64::MAX), budget };
    };
    let light_cache_entries = light_cache_entries(budget);
    let fixed = OTHERS + render + light_cache_entries * crate::LIGHT_CACHE_WORDS * 4;
    let room = (budget as f64 * PLANNED) as u64;
    let fits = |width: u32| 2 * piece_bytes(width, height_m, vpm) + fixed <= room;
    let width_m = WIDTHS_M.iter().copied().filter(|&w| w <= wanted_m).find(|&w| fits(w)).unwrap_or(*WIDTHS_M.last().unwrap());
    Plan { width_m, keep_spare: fits(width_m), light_cache_entries, budget: Some(budget) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1 << 30;

    #[test]
    fn smaller_cards_see_less_far() {
        let render = 110 << 20;
        // What Windows gave the game on a 6 GB card, and about what it'd give on 4 and 2 GB ones
        let six = plan(Some(5152 << 20), 224, 64, 16, render);
        let four = plan(Some(3 * GB + GB / 2), 224, 64, 16, render);
        let two = plan(Some(2 * GB), 224, 64, 16, render);
        assert_eq!(six.width_m, 224);
        assert!(six.keep_spare);
        assert!(four.width_m < 224 && four.width_m >= 160, "{four:?}");
        assert!(two.width_m < four.width_m, "{two:?}");
        assert!(!two.keep_spare, "{two:?}");
        assert!(two.light_cache_entries < six.light_cache_entries);
        // As it turned out on the 6 GB card told it had 2 GB: about 1.6 GB used, 2 GB while moving on
        assert!(piece_bytes(two.width_m, 64, 16) + OTHERS + render + two.light_cache_entries * crate::LIGHT_CACHE_WORDS * 4 <= 3 * GB / 2, "{two:?}");
    }

    #[test]
    fn never_wider_than_wanted() {
        assert_eq!(plan(Some(16 * GB), 160, 64, 16, 0).width_m, 160);
        assert_eq!(plan(None, 224, 64, 16, 0).width_m, 224);
    }
}
