//! Night raids: from the dusk to the dawn, monsters come in waves, from out of sight round the
//! player, and come for them. Behind walls, they find their way round to the doors and beat them
//! down, see bases.rs; only the biggest of them can break walls. At dawn those still about catch
//! fire in the sunlight and burn away. Each night's worse than the last, and worse again the further
//! the player's got through the realms; and once the player's been to a realm, its monsters come at
//! night in the others too, see night_roster().
//!
//! Finding the way: round a base, a grid of its blocks' columns, walls in the way and doors to be
//! broken, is searched out from where the monsters are going, see Nav. Away from anything built,
//! they come straight at the player as monsters always do.

use std::collections::VecDeque;

use crate::animals::Look;
use crate::bases::{BLOCK, Bases, Kind};
use crate::realms::{REALMS, monsters_of};

/// Seconds into the dusk the first wave comes, and between waves after
pub const FIRST_WAVE_SECONDS: f64 = 25.0;
pub const WAVE_SECONDS: f64 = 55.0;
/// How far from the player waves come from, in meters
pub const SPAWN_NEAR_M: f64 = 26.0;
pub const SPAWN_FAR_M: f64 = 36.0;
/// How long those still about at dawn take to burn away, in seconds
pub const BURN_SECONDS: f64 = 5.0;
/// Blocks round what's built near the player the grid of the way to it reaches
const NAV_MARGIN: i64 = 14;
/// How far from the player what's built is looked at for the grid, in blocks
const NAV_REACH: i64 = 60;

/// How hard a night's raid is
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    /// Waves through the night, how many come in each, and the most about at once
    pub waves: u32,
    pub per_wave: u32,
    pub most: u32,
    /// How much tougher than they'd be the monsters are, see Animal::health
    pub toughness: f64,
    /// Whether the biggest come, those that can break walls
    pub brutes: bool,
}

/// The raid on night `night`, the first 0, with `progress` realms opened beyond the first
pub fn plan(night: u32, progress: u32) -> Plan {
    let hard = night + 2 * progress;
    Plan {
        waves: (3 + hard / 2).min(7),
        per_wave: (2 + hard / 2).min(7),
        most: (5 + hard).min(14),
        toughness: 1.0 + 0.12 * hard.min(15) as f64,
        brutes: hard >= 2,
    }
}

/// Which realm, by its number: 0 the normal world, then the sky islands, the dream and the ice,
/// see realms.rs
pub type RealmId = u32;

/// How likely the monsters of a realm other than the one the raid's in are to come in it, all of
/// them together, against the realm's own
const VISITORS: f64 = 4.0;

/// The monsters that come at night in realm `here`, how likely each is and how much tougher than
/// usual: the realm's own, the walking dead and wolves in the normal world, and those of every
/// other realm visited, as tough as in their own; the normal world's own far the most likely
pub fn night_roster(here: RealmId, visited: &[RealmId]) -> Vec<(Look, f64, f64)> {
    let mut roster: Vec<(Look, f64, f64)> = if here == 0 {
        vec![
            (Look::Zombie, 3.0, 1.0),
            (Look::Skeleton, 2.5, 1.0),
            (Look::Ghoul, 1.5, 1.0),
            (Look::ArmoredZombie, 1.2, 1.0),
            (Look::ArmoredSkeleton, 1.0, 1.0),
            (Look::BoneArcher, 1.0, 1.0),
            (Look::Bloater, 0.8, 1.0),
            (Look::Wolf, 1.2, 1.0),
            (Look::GraveBrute, 0.5, 1.0),
        ]
    } else {
        realm_monsters(here, here, 12.0)
    };
    let mut others: Vec<RealmId> = visited.iter().copied().filter(|&r| r != here && (r as usize) < REALMS.len()).collect();
    others.sort_unstable();
    others.dedup();
    for r in others {
        roster.extend(realm_monsters(r, here, VISITORS));
    }
    roster.retain(|(_, w, _)| *w > 0.0);
    roster
}

/// The monsters of realm `realm` come to realm `here`, `weight` in all spread over them by how
/// many of each go about, and how much tougher than here's own: as everything is, they're as tough
/// as realm `here` is, see Game::set_scaling() and realms::scaling_in(), or as their own realm is
/// where that's tougher
fn realm_monsters(realm: RealmId, here: RealmId, weight: f64) -> Vec<(Look, f64, f64)> {
    let (packs, scaling) = monsters_of(realm as usize);
    let (_, home) = monsters_of(here as usize);
    let tougher = (scaling.toughness / home.toughness).max(1.0);
    let count: usize = packs.iter().map(|p| p.looks.len() * p.most).sum();
    packs
        .iter()
        .flat_map(|p| p.looks.iter().map(move |&l| (l, p.most)))
        .map(|(look, most)| (look, weight * most as f64 / count.max(1) as f64, tougher))
        .collect()
}

/// Whether a monster's big enough to break down walls, not just doors
pub fn breaks_walls(look: Look) -> bool {
    look == Look::GraveBrute || look.is_boss() || look.is_raid()
}

/// Picks one of `roster` from `roll`, 0-1, by how likely each is, leaving out the biggest unless
/// `brutes`: which, and how much tougher than usual
pub fn pick(roster: &[(Look, f64, f64)], roll: f64, brutes: bool) -> (Look, f64) {
    let allowed: Vec<&(Look, f64, f64)> = roster.iter().filter(|(l, ..)| brutes || !breaks_walls(*l)).collect();
    let total: f64 = allowed.iter().map(|(_, w, _)| w).sum();
    let mut left = roll.clamp(0.0, 0.999_999) * total;
    for (look, w, tough) in &allowed {
        if left < *w {
            return (*look, *tough);
        }
        left -= w;
    }
    allowed.last().map_or((Look::Zombie, 1.0), |(l, _, t)| (*l, *t))
}

/// What a column of blocks round a base is, for finding the way
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cell {
    Free,
    Wall,
    /// A door shut, by its piece's id
    Door(u32),
}

/// Where a monster's to go: on its way to `at`, or to break down what's at it
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lure {
    Path([f64; 2]),
    Break { at: [f64; 3], door: u32 },
}

/// The way round a base, see the top of this file: block columns from `lo`, `size` across
pub struct Nav {
    lo: [i64; 2],
    size: [i64; 2],
    cells: Vec<Cell>,
    /// Steps to where they're going from each column, u32::MAX for nowhere to go
    steps: Vec<u32>,
    /// Which door each column they're going to is by, if it's by one
    by_door: Vec<Option<u32>>,
    /// Whether the player's shut in, so they go for the doors
    pub enclosed: bool,
    /// The height the player's at, in blocks, for where a door's to be struck
    level: i64,
}

impl Nav {
    /// The way round what's built near the player standing in block `player`, of the whole
    /// landscape: None if nothing's built near
    pub fn new(bases: &Bases, player: [i64; 3]) -> Option<Nav> {
        let near: Vec<_> = bases
            .pieces()
            .iter()
            .filter(|p| (p.at[0] - player[0]).abs() < NAV_REACH && (p.at[1] - player[1]).abs() < NAV_REACH && (p.at[2] - player[2]).abs() < 12)
            .collect();
        if near.is_empty() {
            return None;
        }
        let lo = [near.iter().map(|p| p.at[0]).min()?.min(player[0]) - NAV_MARGIN, near.iter().map(|p| p.at[1]).min()?.min(player[1]) - NAV_MARGIN];
        let hi = [near.iter().map(|p| p.at[0] + 2).max()?.max(player[0] + 1) + NAV_MARGIN, near.iter().map(|p| p.at[1] + 2).max()?.max(player[1] + 1) + NAV_MARGIN];
        let size = [hi[0] - lo[0], hi[1] - lo[1]];
        let mut walls = vec![0u8; (size[0] * size[1]) as usize];
        let mut cells = vec![Cell::Free; (size[0] * size[1]) as usize];
        let index = |x: i64, y: i64| ((x - lo[0]) + (y - lo[1]) * size[0]) as usize;
        let level = player[2];
        for p in &near {
            for b in p.blocks() {
                // What stands in the way of walking at about the player's height
                if b[2] < level - 1 || b[2] > level + 3 {
                    continue;
                }
                let i = index(b[0], b[1]);
                match p.kind {
                    Kind::Door if !p.open => cells[i] = Cell::Door(p.id),
                    Kind::WoodWall | Kind::ClayWall | Kind::StoneWall => walls[i] += 1,
                    _ => {}
                }
            }
        }
        for (i, &w) in walls.iter().enumerate() {
            if w >= 2 && cells[i] == Cell::Free {
                cells[i] = Cell::Wall;
            }
        }
        let mut nav = Nav { lo, size, steps: vec![u32::MAX; cells.len()], by_door: vec![None; cells.len()], cells, enclosed: false, level };
        nav.search(player);
        Some(nav)
    }

    fn index(&self, x: i64, y: i64) -> Option<usize> {
        let (i, j) = (x - self.lo[0], y - self.lo[1]);
        (i >= 0 && j >= 0 && i < self.size[0] && j < self.size[1]).then(|| (i + j * self.size[0]) as usize)
    }

    fn free(&self, x: i64, y: i64) -> bool {
        self.index(x, y).is_some_and(|i| self.cells[i] == Cell::Free)
    }

    /// Works out where they're going and how far each column is from it: whether the player's
    /// shut in, and if they are, the outsides of the doors in their way; if not, the player
    fn search(&mut self, player: [i64; 3]) {
        let n = self.cells.len();
        // What the player can get to without going through a door or a wall
        let mut inside = vec![false; n];
        let mut queue = VecDeque::new();
        if let Some(i) = self.index(player[0], player[1]) {
            inside[i] = true;
            queue.push_back((player[0], player[1]));
        }
        self.enclosed = true;
        while let Some((x, y)) = queue.pop_front() {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                let Some(i) = self.index(nx, ny) else {
                    // Out to the edge: not shut in
                    self.enclosed = false;
                    continue;
                };
                if !inside[i] && self.cells[i] == Cell::Free {
                    inside[i] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
        let mut queue = VecDeque::new();
        if self.enclosed {
            // The columns just outside each door, from where it's struck
            for y in self.lo[1]..self.lo[1] + self.size[1] {
                for x in self.lo[0]..self.lo[0] + self.size[0] {
                    let i = self.index(x, y).unwrap();
                    let Cell::Door(id) = self.cells[i] else { continue };
                    let touches_inside = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| self.index(x + dx, y + dy).is_some_and(|j| inside[j]));
                    if !touches_inside {
                        continue;
                    }
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        if let Some(j) = self.index(x + dx, y + dy)
                            && self.cells[j] == Cell::Free
                            && !inside[j]
                            && self.steps[j] != 0
                        {
                            self.steps[j] = 0;
                            self.by_door[j] = Some(id);
                            queue.push_back((x + dx, y + dy));
                        }
                    }
                }
            }
        } else if let Some(i) = self.index(player[0], player[1]) {
            self.steps[i] = 0;
            queue.push_back((player[0], player[1]));
        }
        // Out from there, round walls, not cutting their corners
        while let Some((x, y)) = queue.pop_front() {
            let here = self.steps[self.index(x, y).unwrap()];
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if dx != 0 && dy != 0 && !(self.free(x + dx, y) && self.free(x, y + dy)) {
                    continue;
                }
                let Some(j) = self.index(nx, ny) else { continue };
                if self.cells[j] != Cell::Free || (self.enclosed && inside[j]) || self.steps[j] != u32::MAX {
                    continue;
                }
                self.steps[j] = here + 1;
                self.by_door[j] = self.by_door[self.index(x, y).unwrap()];
                queue.push_back((nx, ny));
            }
        }
    }

    /// Where a monster at `at`, in voxels of the whole landscape, is to go: None to come straight
    /// at the player, as it's outside the grid and they're not shut in, or nothing's in the way
    pub fn lure(&self, at: [f64; 3], bases: &Bases) -> Option<Lure> {
        let (x, y) = ((at[0] / BLOCK as f64).floor() as i64, (at[1] / BLOCK as f64).floor() as i64);
        let middle = |x: i64, y: i64| [(x as f64 + 0.5) * BLOCK as f64, (y as f64 + 0.5) * BLOCK as f64];
        let Some(i) = self.index(x, y) else {
            // Outside the grid: towards its middle if the player's shut in, straight at them if not
            return self.enclosed.then(|| Lure::Path(middle(self.lo[0] + self.size[0] / 2, self.lo[1] + self.size[1] / 2)));
        };
        // Pressed right up into a door, it beats on it; caught on a wall, it steps off it the way
        // that leads on
        if let Cell::Door(door) = self.cells[i]
            && self.enclosed
        {
            let m = bases.piece(door)?.middle();
            return Some(Lure::Break { at: [m[0], m[1], (self.level * BLOCK) as f64], door });
        }
        if self.cells[i] == Cell::Wall {
            let next = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter_map(|(dx, dy)| Some((x + dx, y + dy, self.steps[self.index(x + dx, y + dy)?]))).filter(|n| n.2 != u32::MAX).min_by_key(|n| n.2);
            return next.map(|(nx, ny, _)| Lure::Path(middle(nx, ny)));
        }
        let steps = self.steps[i];
        if steps == u32::MAX {
            return None;
        }
        if steps == 0 {
            if !self.enclosed {
                return None;
            }
            let door = self.by_door[i]?;
            let piece = bases.piece(door)?;
            let m = piece.middle();
            return Some(Lure::Break { at: [m[0], m[1], (self.level * BLOCK) as f64], door });
        }
        // A few steps on down the way, for it to head straight for
        let (mut cx, mut cy) = (x, y);
        for _ in 0..3 {
            let here = self.steps[self.index(cx, cy).unwrap()];
            let next = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)]
                .iter()
                .filter(|(dx, dy)| !(*dx != 0 && *dy != 0) || (self.free(cx + dx, cy) && self.free(cx, cy + dy)))
                .filter_map(|(dx, dy)| Some((cx + dx, cy + dy, self.steps[self.index(cx + dx, cy + dy)?])))
                .filter(|&(_, _, s)| s < here)
                .min_by_key(|&(_, _, s)| s);
            match next {
                Some((nx, ny, _)) => (cx, cy) = (nx, ny),
                None => break,
            }
        }
        // Nothing in the way of coming straight at the player
        if !self.enclosed && steps <= 3 {
            return None;
        }
        Some(Lure::Path(middle(cx, cy)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bases::Materials;

    /// A hut of walls two high, six blocks square, with a door in its front, at block 100, 100, 10
    fn hut(bases: &mut Bases) -> u32 {
        bases.materials = Materials::new(1000, 1000, 1000);
        for j in 0..6 {
            for i in 0..6 {
                if i != 0 && j != 0 && i != 5 && j != 5 {
                    continue;
                }
                for k in 0..4 {
                    if j == 0 && (i == 2 || i == 3) {
                        continue;
                    }
                    bases.build(Kind::StoneWall, [100 + i, 100 + j, 10 + k], 0).unwrap();
                }
            }
        }
        bases.build(Kind::Door, [102, 100, 10], 3).unwrap();
        bases.piece_at([102, 100, 10]).unwrap().id
    }

    // Shut in, they go round the walls to the outside of the door and beat on it; with it open,
    // straight in at the player
    #[test]
    fn the_way_round_to_the_door() {
        let mut bases = Bases::new();
        let door = hut(&mut bases);
        let inside = [102, 102, 10];
        let nav = Nav::new(&bases, inside).unwrap();
        assert!(nav.enclosed);
        let voxel = |x: i64, y: i64| [(x as f64 + 0.5) * 8.0, (y as f64 + 0.5) * 8.0, 80.0];
        // Right outside the door: break it
        match nav.lure(voxel(102, 99), &bases) {
            Some(Lure::Break { door: d, .. }) => assert_eq!(d, door),
            other => panic!("{other:?}"),
        }
        // Behind the hut: led round it, never through a wall
        let mut at = voxel(102, 108);
        for _ in 0..40 {
            match nav.lure(at, &bases) {
                Some(Lure::Path(p)) => {
                    let (x, y) = ((p[0] / 8.0).floor() as i64, (p[1] / 8.0).floor() as i64);
                    assert!(!(100..106).contains(&x) || !(100..106).contains(&y), "led through the hut to {x}, {y}");
                    at = [p[0], p[1], 80.0];
                }
                Some(Lure::Break { door: d, .. }) => {
                    assert_eq!(d, door);
                    break;
                }
                None => panic!("lost its way"),
            }
        }
        assert!(matches!(nav.lure(at, &bases), Some(Lure::Break { .. })), "never got to the door");
        // With the door open the player's not shut in, and they come straight on once near
        bases.toggle_door(door);
        let nav = Nav::new(&bases, inside).unwrap();
        assert!(!nav.enclosed);
        assert!(matches!(nav.lure(voxel(102, 108), &bases), Some(Lure::Path(_))), "still led round the walls");
        assert_eq!(nav.lure(voxel(102, 101), &bases), None);
        // Outside, nothing in the way
        let nav = Nav::new(&bases, [90, 90, 10]).unwrap();
        assert!(!nav.enclosed);
        // Far from anything built, no grid at all
        assert!(Nav::new(&bases, [400, 400, 10]).is_none());
    }

    // Each night's harder than the last, and more so further through the realms
    #[test]
    fn nights_get_harder() {
        let first = plan(0, 0);
        assert!(!first.brutes && first.waves >= 3 && first.per_wave >= 2);
        let later = plan(6, 0);
        assert!(later.per_wave > first.per_wave && later.most > first.most && later.toughness > first.toughness && later.brutes);
        assert!(plan(2, 2).toughness > plan(2, 0).toughness);
        // The first night has nothing that breaks walls
        let roster = night_roster(0, &[0]);
        for k in 0..100 {
            assert!(!breaks_walls(pick(&roster, k as f64 / 100.0, false).0));
        }
        assert!((0..100).any(|k| breaks_walls(pick(&roster, k as f64 / 100.0, true).0)));
        assert!(breaks_walls(Look::GraveBrute) && !breaks_walls(Look::Zombie));
    }

    // Once a realm's been visited, its monsters come at night in the others too, as tough as in
    // their own, but the realm's own come the most
    #[test]
    fn monsters_of_realms_visited_come_too() {
        let home = night_roster(0, &[0]);
        assert!(home.iter().all(|&(_, _, t)| t == 1.0));
        let travelled = night_roster(0, &[0, 1, 2]);
        let sky = monsters_of(1).0.iter().flat_map(|p| p.looks.iter()).copied().collect::<Vec<_>>();
        let dream = monsters_of(2).0.iter().flat_map(|p| p.looks.iter()).copied().collect::<Vec<_>>();
        assert!(travelled.iter().any(|(l, _, t)| sky.contains(l) && *t > 1.0), "nothing from the sky isles");
        assert!(travelled.iter().any(|(l, _, t)| dream.contains(l) && *t > 1.0), "nothing from the dream");
        let own: f64 = travelled.iter().filter(|(_, _, t)| *t == 1.0).map(|(_, w, _)| w).sum();
        let all: f64 = travelled.iter().map(|(_, w, _)| w).sum();
        assert!(own > all * 0.5, "the normal world's own come the most");
        // Not visited, none of them
        assert!(!home.iter().any(|(l, ..)| *l == Look::Gloomwing || *l == Look::RimeWraith));
        // A realm's raid has its own monsters, as tough as everything there is already, not tougher
        // again, and those of realms before it no less tough
        let ice = night_roster(3, &[0, 3]);
        assert!(ice.iter().any(|(l, ..)| *l == Look::RimeWraith));
        assert!(ice.iter().all(|&(_, _, t)| t == 1.0), "{ice:?}");
    }
}
