//! How the graphics card's memory is shared out, so the game never needs more than Windows gives
//! it. Past that, Windows moves some of it out to the computer's memory: every frame gets slower,
//! and the strips the world moves on by can't be sent over in time, so the world's seen to end.
//!
//! Chosen once at the start, from how much there is, see plan(): how wide the piece of the
//! landscape kept around the player is, and so how far one sees, how many bricks there's room for,
//! and how big the light cache is. The render scale is kept within what's left as the game goes
//! on, see App::render_room() in main.rs.
//!
//! There's only ever the one piece: it moves on over the landscape a strip at a time, see
//! stream.rs, with room for a strip more each way for the next to come into.
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

/// The widths the piece of the landscape kept around the player can be, in meters, widest first,
/// each a whole number of strips, see stream.rs. One sees half of it, less a strip and
/// FADE_SHORT_M in main.rs, before things fade into the sky.
pub const WIDTHS_M: &[u32] = &[224, 208, 192, 176, 160, 144, 128];

/// Stored bricks per column of voxels: pieces 224 m wide at 16 voxels a meter had 0.06 (765,000
/// bricks) to 0.11 (1.45 million, with a fallen city under it). Planned for near the most.
const BRICKS_PER_COLUMN: f64 = 0.1;

/// How many bricks the GPU has room for to start with, in a piece of the landscape `width_m` wide
/// at `vpm` voxels a meter: its own and the next strip's as it comes in, see BRICKS_PER_COLUMN,
/// with room for holes blown in it
pub fn piece_bricks(width_m: u32, vpm: u32) -> usize {
    let columns = (width_m * vpm) as u64 * ((width_m + STRIP_M) * vpm) as u64;
    (columns as f64 * BRICKS_PER_COLUMN) as usize + worldgen::SPARE_BRICKS
}

/// How wide the strips the piece moves on by are, see stream.rs
const STRIP_M: u32 = crate::stream::STRIP_M;

/// About how much of the graphics card's memory a piece of the landscape `width_m` wide and
/// `height_m` tall takes at `vpm` voxels a meter, with the room it has for a strip more each way:
/// its bricks with room for holes blown in it, the map of them, the grass, and the water's ground,
/// depth, flow and surface (the last two in half floats, see water.rs)
pub fn piece_bytes(width_m: u32, height_m: u32, vpm: u32) -> u64 {
    let side = ((width_m + STRIP_M) * vpm) as u64;
    let room = side * side;
    let grid = room / 64 * (height_m * vpm / 8) as u64 * 4;
    bricks_bytes(width_m, vpm) + grid + room * 4 + room * (4 + 4 + 8 + 8)
}

/// About how many bytes the bricks of a piece of the landscape `width_m` wide at `vpm` voxels a
/// meter take, see piece_bricks(): all in one buffer, which traverse.comp sees whole
pub fn bricks_bytes(width_m: u32, vpm: u32) -> u64 {
    piece_bricks(width_m, vpm) as u64 * worldgen::BRICK_WORDS as u64 * 4
}

/// The most bricks a world can have room for on the graphics card: as many as a shader can see
/// of one buffer, 1 GB's worth on Intel's integrated graphics, see limits.rs
pub fn most_bricks() -> usize {
    (crate::limits::get().buffer_bytes() / (worldgen::BRICK_WORDS as u64 * 4)) as usize
}

/// Whether the graphics card can hold a piece of the landscape `width_m` wide and `height_m` tall
/// at `vpm` voxels a meter at all, however much memory it has: its bricks in a buffer a shader sees
/// whole, its grid of them in a 3D image and its columns in 2D ones, with room for a strip more
/// each way, see limits.rs
fn bindable(width_m: u32, height_m: u32, vpm: u32) -> bool {
    let limits = crate::limits::get();
    let side = (width_m + STRIP_M) * vpm;
    bricks_bytes(width_m, vpm) <= limits.buffer_bytes() && side <= limits.image_2d && side / 8 <= limits.image_3d && height_m * vpm / 8 <= limits.image_3d
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
    /// How many bricks the GPU has room for to start with, see piece_bricks()
    pub bricks: usize,
    /// Entries in the light cache, see LIGHT_CACHE_WORDS in main.rs
    pub light_cache_entries: u64,
    /// What the graphics card has for the game, in bytes, if it could say
    pub budget: Option<u64>,
}

/// How the graphics card's memory is shared out, for `budget` bytes of it, None if unknown, with a
/// piece of the landscape `wanted_m` wide wanted, `height_m` tall at `vpm` voxels a meter, and
/// `render` bytes for what's rendered into at the usual render scale: the widest piece that fits
/// beside the rest, and no wider than wanted, or the narrowest there is. There's only ever the
/// one, moving on a strip at a time.
pub fn plan(budget: Option<u64>, wanted_m: u32, height_m: u32, vpm: u32, render: u64) -> Plan {
    // Nor wider than the card can hold at all, see bindable()
    let narrowest = *WIDTHS_M.last().unwrap();
    let width = |width_m: u32, budget: Option<u64>| Plan { width_m, bricks: piece_bricks(width_m, vpm), light_cache_entries: light_cache_entries(budget.unwrap_or(u64::MAX)), budget };
    let Some(budget) = budget else {
        let width_m = if bindable(wanted_m, height_m, vpm) { wanted_m } else { WIDTHS_M.iter().copied().find(|&w| w <= wanted_m && bindable(w, height_m, vpm)).unwrap_or(narrowest) };
        return width(width_m, None);
    };
    let light_cache_entries = light_cache_entries(budget);
    let fixed = OTHERS + render + light_cache_entries * crate::LIGHT_CACHE_WORDS * 4;
    let room = (budget as f64 * PLANNED) as u64;
    let fits = |width: u32| piece_bytes(width, height_m, vpm) + fixed <= room;
    let width_m = WIDTHS_M.iter().copied().filter(|&w| w <= wanted_m && bindable(w, height_m, vpm)).find(|&w| fits(w)).unwrap_or(narrowest);
    width(width_m, Some(budget))
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
        // With only the one piece, a 4 GB card has room for all of it, and a 2 GB one for far more
        // than the 128 m it had when moving on took two
        assert_eq!(four.width_m, 224, "{four:?}");
        assert!(two.width_m < four.width_m && two.width_m >= 160, "{two:?}");
        assert!(two.light_cache_entries < six.light_cache_entries);
        // Room for at least as many bricks as the most a piece has had, 0.11 a column
        let columns = (two.width_m * 16) as usize * ((two.width_m + STRIP_M) * 16) as usize;
        assert!(two.bricks as f64 >= columns as f64 * 0.1, "{two:?}");
        // Every width moves on a whole number of strips
        assert!(WIDTHS_M.iter().all(|w| w % STRIP_M == 0));
    }

    // On AMD's and Intel's cards, see limits::MIN, the widest piece fits however full of bricks
    // it's planned for, and the most a piece has been seen to have, with a fallen city under it,
    // 1.45 million, and all its maps fit in their images
    #[test]
    fn widest_pieces_fit_amd_and_intel() {
        let most = crate::limits::MIN.buffer_bytes();
        assert!(bricks_bytes(224, 16) <= most);
        assert!((1_450_000 + worldgen::SPARE_BRICKS as u64) * worldgen::BRICK_WORDS as u64 * 4 <= most);
        assert!(bindable(224, 64, 16));
        assert_eq!(plan(None, 224, 64, 16, 0).width_m, 224);
        assert!(most_bricks() >= 1_450_000 + worldgen::SPARE_BRICKS);
    }

    #[test]
    fn never_wider_than_wanted() {
        assert_eq!(plan(Some(16 * GB), 160, 64, 16, 0).width_m, 160);
        assert_eq!(plan(None, 224, 64, 16, 0).width_m, 224);
    }
}
