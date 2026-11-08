//! The Innert Gate and the portals home as the player builds them, see realms.rs and session.rs.
//! A new game of Realms has no gate: a ring of its old foundation stones lies where it stood, and
//! the player gathers what it takes and builds it anew in building (B), wherever they like in the
//! normal world. Building it again elsewhere moves it; what's been set into it is kept with the
//! journey, not with it, so nothing's lost. In each realm past the normal world they can build a
//! portal home of their own as well, which they come back to the next time they travel there.

use std::f64::consts::PI;

use crate::bases::{self, BLOCK, Kind, Materials};
use crate::building::Ghost;
use crate::realms::{Places, REALMS};
use crate::save::{GateSave, Mode, PortalSave, RealmsSave};
use crate::session::realm_of;

use super::*;

/// What the gate costs to build: wood, stone and clay
pub const GATE_COST: Materials = Materials::new(20, 60, 30);
/// And a portal home, in a realm past the normal world
pub const PORTAL_COST: Materials = Materials::new(10, 20, 10);
/// How far from the eye the gate or a portal can be put, in meters: further than a wall, being big
pub const GATE_REACH_M: f64 = 14.0;
/// How far in front of a gate or portal the player built they come out of it, in meters
const BUILT_ARRIVAL_M: f64 = 4.5;
/// Half the gate's dais, the base it stands on, front to back and across, in meters, see
/// animals::gate(): what has to be clear and near enough level to build it on
pub const FOOT: [f64; 2] = [2.3, 2.9];
/// Most the ground under the dais can rise and fall, in meters: it's sunk into the ground deeper
/// than that, see animals::GATE_SINK
const LEVEL_M: f64 = 1.2;
/// The ring of old foundation stones where the gate stood: from how far out from its middle to
/// how far, in meters, and the way in at its front, so far round either way of it, in radians
const RUIN_RING_M: [f64; 2] = [4.4, 5.3];
const RUIN_DOORWAY: f64 = 0.45;

/// The places of realm `found.realm` as the player's built them: the gate or the portal home where
/// they built it, coming out in front of it, and whether there's one standing at all. Without one
/// of their own a realm's portal home stands where it was found; the gate stands only once built,
/// or where it was found in games from before it could be.
pub fn stand(found: &Places, progress: &RealmsSave) -> (Places, bool) {
    let mut places = found.clone();
    let built = if places.realm == 0 {
        match progress.gate {
            GateSave::Unbuilt => return (places, false),
            GateSave::Found => None,
            GateSave::Built { at, facing } => Some((at, facing)),
        }
    } else {
        progress.portals.iter().find(|p| p.realm as usize == places.realm).map(|p| (p.at, p.facing))
    };
    if let Some((at, facing)) = built {
        places.gate = at;
        places.facing = facing;
        places.arrival = [at[0] + facing.cos() * BUILT_ARRIVAL_M, at[1] + facing.sin() * BUILT_ARRIVAL_M];
    }
    (places, true)
}

/// Builds the gate, or for `home` the portal home of the realm the player's in, at `at` facing
/// `facing`, both in meters of the whole landscape: where it is in `progress`, and for the gate
/// whether one stood before, which it's moved from
pub fn build_in(progress: &mut RealmsSave, home: bool, at: [f64; 2], facing: f64) -> bool {
    if home {
        let realm = progress.realm;
        let before = progress.portals.iter().any(|p| p.realm == realm);
        progress.portals.retain(|p| p.realm != realm);
        progress.portals.push(PortalSave { realm, at, facing });
        before
    } else {
        let before = progress.gate != GateSave::Unbuilt;
        progress.gate = GateSave::Built { at, facing };
        before
    }
}

impl App {
    /// What can be built besides Kind::ALL where the player is: in Realms the gate in the normal
    /// world, Some(false), or a portal home in a realm past it, Some(true)
    pub(super) fn gate_choice(&self) -> Option<bool> {
        if self.session.mode != Some(Mode::Realms) {
            return None;
        }
        realm_of(&self.session).map(|realm| realm > 0)
    }

    /// The top of the ground at `x`, `y` in voxels of the world, looking from a few meters over
    /// `near` to a few under it
    fn ground_near(&self, x: f64, y: f64, near: f64) -> Option<f64> {
        let vpm = self.world.voxels_per_meter as f64;
        let (xi, yi) = (x.floor() as i64, y.floor() as i64);
        let [sx, sy, sz] = self.world.size.map(|n| n as i64);
        if xi < 0 || yi < 0 || xi >= sx || yi >= sy {
            return None;
        }
        let top = ((near + 4.0 * vpm) as i64).min(sz - 1);
        let bottom = ((near - 8.0 * vpm) as i64).max(0);
        (bottom..=top).rev().find(|&z| !worldgen::is_passable(self.world.material_at(xi, yi, z)) && !gather::is_trunk(self.world.material_at(xi, yi, z))).map(|z| z as f64 + 1.0)
    }

    /// The gate or a portal home put where the player's looking, facing them, and whether it can
    /// be built there
    pub(super) fn gate_ghost(&self) -> Option<Ghost> {
        let home = self.gate_choice()?;
        let (eye, look) = self.eye_look();
        let vpm = self.world.voxels_per_meter as f64;
        let (hit, _) = gather::raycast(&self.world, eye, look, GATE_REACH_M * vpm)?;
        let (x, y) = (hit[0] as f64 + 0.5, hit[1] as f64 + 0.5);
        let z = self.ground_near(x, y, hit[2] as f64)?;
        let at = Vector3::new(x, y, z);
        let facing = (eye.y - y).atan2(eye.x - x) + self.building.turn as f64 * FRAC_PI_2;
        let ok = self.can_build_gate(at, facing, home);
        Some(Ghost::Gate { at, facing, home, ok })
    }

    /// Whether the gate, or a portal home, can be built with its foot at `at`, in voxels of the
    /// world, facing `facing`: level ground clear of water and of what's built, and enough to build
    /// it with
    fn can_build_gate(&self, at: Vector3<f64>, facing: f64, home: bool) -> Result<(), String> {
        let vpm = self.world.voxels_per_meter as f64;
        let (c, s) = (facing.cos(), facing.sin());
        let margin = 2.0 * self.stream.as_ref().map_or(0.0, |st| st.strip as f64);
        let [sx, sy, _] = self.world.size.map(|n| n as f64);
        if at.x < margin || at.y < margin || at.x > sx - margin || at.y > sy - margin {
            return Err("That's too far out.".into());
        }
        // The ground under the dais near enough level, at its middle, corners and sides
        let mut heights = Vec::new();
        for (u, v) in [(0.0, 0.0), (1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            let (lx, ly) = (u * FOOT[0] * vpm, v * FOOT[1] * vpm);
            let (x, y) = (at.x + lx * c - ly * s, at.y + lx * s + ly * c);
            let h = self.ground_near(x, y, at.z).ok_or("The ground's too uneven here.")?;
            if self.water.depth_at(x, y) > 0.3 * vpm {
                return Err("Not in the water.".into());
            }
            heights.push(h);
        }
        let (low, high) = heights.iter().fold((f64::MAX, f64::MIN), |(a, b), &h| (a.min(h), b.max(h)));
        if high - low > LEVEL_M * vpm {
            return Err("The ground's too uneven here.".into());
        }
        // Nothing built where it'd stand
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let reach = [(FOOT[0] + 0.4) * vpm, (FOOT[1] + 0.4) * vpm];
        let in_the_way = self.bases.pieces().iter().any(|p| {
            let m = Vector3::from(p.middle()) - origin - at;
            let (along, across) = (m.x * c + m.y * s, -m.x * s + m.y * c);
            along.abs() < reach[0] && across.abs() < reach[1] && m.z > -2.0 * vpm && m.z < 8.0 * vpm
        });
        if in_the_way {
            return Err("Something's built in the way.".into());
        }
        let cost = if home { PORTAL_COST } else { GATE_COST };
        if !self.bases.materials.covers(cost) {
            return Err(format!("Needs {}.", cost.describe()));
        }
        Ok(())
    }

    /// Builds the gate, or a portal home, where the ghost shows it, if it can be: moving the one
    /// there was, and giving back half of what it cost, as its stones are taken along
    pub(super) fn build_gate_here(&mut self) {
        let Some(Ghost::Gate { at, facing, home, ok }) = self.gate_ghost() else {
            return;
        };
        if let Err(why) = ok {
            self.game.say(why);
            return;
        }
        self.build_gate(at, facing, home);
    }

    /// Builds the gate, or for `home` a portal home, with its foot at `at`, in voxels of the
    /// world, facing `facing`, paying for it
    pub(super) fn build_gate(&mut self, at: Vector3<f64>, facing: f64, home: bool) {
        let cost = if home { PORTAL_COST } else { GATE_COST };
        if !self.bases.materials.take(cost) {
            return;
        }
        let vpm = self.world.voxels_per_meter as f64;
        let meters = [(at.x + self.world.origin[0] as f64) / vpm, (at.y + self.world.origin[1] as f64) / vpm];
        let Some(progress) = self.session.realms.as_mut() else {
            return;
        };
        let moved = build_in(progress, home, meters, facing);
        if moved {
            self.bases.materials.add(cost.share(0.5));
        }
        self.refresh_gate();
        let name = if home { "portal home" } else { "Innert Gate" };
        println!("GATE built: the {name} at {:.1}, {:.1} m facing {:.2}{}", meters[0], meters[1], facing, if moved { ", moved" } else { "" });
        self.game.say(match (home, moved) {
            (false, false) => "The Innert Gate stands again! Set each realm's three Innert blocks into it to open the way on (E).".to_owned(),
            (false, true) => "The Innert Gate is moved here, every block set into it with it. Half its stones came back.".to_owned(),
            (true, false) => format!("A portal home of your own stands here. It takes you to the Innert Gate, and brings you back here to {}.", REALMS[realm_of(&self.session).unwrap_or(0)].name),
            (true, true) => "The portal home is moved here. Half its stones came back.".to_owned(),
        });
        // Dust and chips round its foot, and a deep toll
        for k in 0..8 {
            let a = k as f64 / 8.0 * TAU;
            let p = at + Vector3::new(a.cos() * 2.0 * vpm, a.sin() * 2.0 * vpm, 0.5 * vpm);
            self.game.debris(p, &[worldgen::material::STONE, worldgen::material::GRAVEL, worldgen::material::MOSS], vpm);
        }
        let view = self.view();
        if let Some(audio) = &mut self.audio {
            audio.play(audio::Cue::at(audio::Sound::Toll, at + Vector3::new(0.0, 0.0, 3.0 * vpm), 1.0).pitched(if home { 1.2 } else { 0.8 }), &view);
            audio.play(audio::Cue::at(audio::Sound::Place, at, 1.0).pitched(0.6), &view);
        }
        self.save_game(false);
    }

    /// Takes up where the gate or portal home is now, after it's built or moved
    pub(super) fn refresh_gate(&mut self) {
        let (Some(found), Some(progress)) = (&self.session.found, &self.session.realms) else {
            return;
        };
        let (places, standing) = stand(found, progress);
        self.session.places = Some(places);
        self.session.standing = standing;
        self.refresh_shrines();
    }

    /// In a new game of Realms, the ring of the gate's old foundation stones laid where it stood,
    /// once the world's there round it: weathered stones one to three blocks high in a ring two
    /// thick, broken here and there, with a way in at its front
    pub(super) fn lay_ruins(&mut self) {
        let wanted = self.session.realms.as_ref().is_some_and(|r| r.gate == GateSave::Unbuilt && !r.ruins && r.realm == 0);
        if !wanted || self.held_still() || self.session.arriving || self.stream.as_ref().is_some_and(|s| s.teleport.is_some()) {
            return;
        }
        let Some(found) = self.session.found.clone() else {
            return;
        };
        let vpm = self.world.voxels_per_meter as f64;
        let [ox, oy] = [self.world.origin[0] as f64, self.world.origin[1] as f64];
        let middle = Vector3::new(found.gate[0] * vpm - ox, found.gate[1] * vpm - oy, 0.0);
        let margin = 2.0 * self.stream.as_ref().map_or(0.0, |s| s.strip as f64) + 8.0 * vpm;
        let [sx, sy, _] = self.world.size.map(|n| n as f64);
        if middle.x < margin || middle.y < margin || middle.x > sx - margin || middle.y > sy - margin {
            return;
        }
        let ground = game::ground_top(&self.world, middle.x, middle.y);
        let block = BLOCK as f64;
        let reach = (RUIN_RING_M[1] * vpm / block).ceil() as i64 + 1;
        let center = [((middle.x + ox) / block).floor() as i64, ((middle.y + oy) / block).floor() as i64];
        let mut laid = 0;
        for j in -reach..=reach {
            for i in -reach..=reach {
                let b = [center[0] + i, center[1] + j];
                // The middle of the column, in meters from the ring's
                let (dx, dy) = (((b[0] as f64 + 0.5) * block - ox - middle.x) / vpm, ((b[1] as f64 + 0.5) * block - oy - middle.y) / vpm);
                let r = dx.hypot(dy);
                if r < RUIN_RING_M[0] || r > RUIN_RING_M[1] {
                    continue;
                }
                // A way in at the front, and stones fallen away here and there
                let a = dy.atan2(dx);
                let off = (a - found.facing + PI).rem_euclid(TAU) - PI;
                if off.abs() < RUIN_DOORWAY {
                    continue;
                }
                let roll = bases::hash3(b[0] as u32, b[1] as u32, 0x0ad5);
                let arc = bases::hash3(((a + PI) / TAU * 9.0) as u32, 0x51e5, 3);
                if arc % 5 == 0 || roll % 9 == 0 {
                    continue;
                }
                let (x, y) = ((b[0] as f64 + 0.5) * block - ox, (b[1] as f64 + 0.5) * block - oy);
                let Some(top) = self.ground_near(x, y, ground) else { continue };
                // Sunk a little into the ground, one to three blocks high, higher in some arcs
                let foot = (top as i64 - 3 + self.world.origin[2]).div_euclid(BLOCK);
                let high = 1 + (roll % 2) as i64 + (arc % 3 == 1) as i64;
                for k in 0..high {
                    let turn = if k == high - 1 { 1 } else { 0 };
                    if self.bases.build(Kind::OldStone, [b[0], b[1], foot + k], turn).is_ok() {
                        laid += 1;
                    }
                }
            }
        }
        if let Some(progress) = self.session.realms.as_mut() {
            progress.ruins = true;
        }
        self.refresh_bases();
        println!("RUINS: {laid} old foundation stones laid round {:.1}, {:.1} m", found.gate[0], found.gate[1]);
    }
}

impl App {
    /// For screenshot runs, see building::SCRIPTS. `gate`, in a new game of Realms, looks at the
    /// old foundations once they're laid, picks the gate in building and aims it in among them,
    /// builds it, sets blocks into it, and builds it again further off to move it, a shot at each.
    /// `portal`, set out for a realm with `--set realm=1`, builds a portal home off to one side,
    /// goes home and comes back, and looks at where they come out.
    pub(super) fn gate_script(&mut self, name: &str, frame: usize) {
        let vpm = self.world.voxels_per_meter as f64;
        let [ox, oy] = [self.world.origin[0] as f64, self.world.origin[1] as f64];
        let settled = !self.held_still() && !self.session.arriving && self.stream.as_ref().is_none_or(|s| s.teleport.is_none());
        let since = frame.saturating_sub(self.building.mark);
        let up = |m: f64| Vector3::new(0.0, 0.0, m * vpm);
        let on_ground = |app: &App, x: f64, y: f64| Vector3::new(x, y, game::ground_top(&app.world, x, y));
        let next = |app: &mut App, phase: u8| (app.building.phase, app.building.mark) = (phase, frame);
        let Some(found) = self.session.found.clone() else {
            return;
        };
        let front = Vector3::new(found.facing.cos(), found.facing.sin(), 0.0);
        let side = Vector3::new(-front.y, front.x, 0.0);
        let site = on_ground(self, found.gate[0] * vpm - ox, found.gate[1] * vpm - oy);
        // Open ground near `near`, in voxels of the world
        let open = |app: &App, near: Vector3<f64>| {
            app.open_ground(near, 12).map(|b| {
                let (x, y) = ((b[0] + 6) as f64 * BLOCK as f64 - ox, (b[1] + 6) as f64 * BLOCK as f64 - oy);
                on_ground(app, x, y)
            })
        };
        let pick_gate = |app: &mut App| {
            app.building.on = true;
            app.building.picked = Kind::ALL.len();
            app.building.turn = 0;
        };
        match (name, self.building.phase) {
            ("gate", 0) if settled && self.session.realms.as_ref().is_some_and(|r| r.ruins) && frame > 30 => {
                let label = self.compass().map(|b| b.label);
                println!("frame {frame}: the old foundations at {:.1}, {:.1} m, {} stones, the compass to {label:?}", found.gate[0], found.gate[1], self.bases.pieces().len());
                self.look_from(site + front * 11.0 * vpm + up(3.5), site + up(0.6));
                next(self, 1);
            }
            ("gate", 1) if since == 60 => self.shoot_soon("the old foundations"),
            ("gate", 1) if since == 70 => {
                self.bases.materials.add(Materials::new(GATE_COST.wood * 3, GATE_COST.stone * 3, GATE_COST.clay * 3));
                pick_gate(self);
                self.look_from(site + front * 9.0 * vpm + up(1.7), site);
                next(self, 2);
            }
            ("gate", 2) if since == 40 => self.shoot_soon("where it'd go"),
            ("gate", 2) if since == 50 => {
                self.build_gate_here();
                println!("frame {frame}: built, standing {}, carrying {:?}", self.session.standing, self.bases.materials);
                self.look_from(site + front * 14.0 * vpm + up(4.0), site + up(3.0));
                next(self, 3);
            }
            ("gate", 3) if since == 60 => self.shoot_soon("the Innert Gate built"),
            ("gate", 3) if since == 70 => {
                // A realm's blocks set into it, and somewhere else to build it again
                if let Some(progress) = self.session.realms.as_mut() {
                    progress.placed = vec![3, 3, 1];
                }
                self.refresh_shrines();
                let Some(spot) = open(self, site + side * 26.0 * vpm) else {
                    println!("frame {frame}: no open ground to move it to");
                    return next(self, 9);
                };
                self.building.spot = Some(spot);
                self.look_from(spot + front * 9.0 * vpm + up(1.7), spot);
                next(self, 4);
            }
            ("gate", 4) if since == 20 => {
                self.build_gate_here();
                let spot = self.building.spot.unwrap_or(site);
                let places = self.session.places.clone();
                println!("frame {frame}: moved, the gate at {:?}, carrying {:?}, placed {:?}", places.map(|p| p.gate), self.bases.materials, self.session.realms.as_ref().map(|r| r.placed.clone()));
                self.look_from(spot + front * 8.0 * vpm + side * 2.0 * vpm + up(3.0), spot + up(3.0));
                next(self, 5);
            }
            ("gate", 5) if since == 60 => self.shoot_soon("moved, its blocks with it"),
            ("gate", 5) if since == 70 => {
                // Where it stood before, only the old stones left
                self.building.on = false;
                self.look_from(site + front * 11.0 * vpm + up(3.5), site + up(0.6));
            }
            ("gate", 5) if since == 130 => self.shoot_soon("where it stood before"),
            ("portal", 0) if settled && frame > 40 => {
                let realm = realm_of(&self.session).unwrap_or(0);
                if realm == 0 {
                    println!("frame {frame}: in the normal world, --set realm=1 to try a portal home");
                    return next(self, 9);
                }
                self.building.back_to = realm;
                self.bases.materials.add(Materials::new(PORTAL_COST.wood * 3, PORTAL_COST.stone * 3, PORTAL_COST.clay * 3));
                let Some(spot) = open(self, site + side * 18.0 * vpm) else {
                    println!("frame {frame}: no open ground for a portal home");
                    return next(self, 9);
                };
                pick_gate(self);
                self.building.spot = Some(spot);
                self.look_from(spot + front * 9.0 * vpm + up(1.7), spot);
                next(self, 1);
            }
            ("portal", 1) if since == 40 => self.shoot_soon("where the portal home would go"),
            ("portal", 1) if since == 50 => {
                self.build_gate_here();
                let spot = self.building.spot.unwrap_or(site);
                println!("frame {frame}: portal home built, the realm's places {:?}", self.session.places.as_ref().map(|p| (p.gate, p.arrival)));
                self.look_from(spot + front * 10.0 * vpm + up(3.0), spot + up(2.5));
                next(self, 2);
            }
            ("portal", 2) if since == 60 => self.shoot_soon("the portal home built"),
            ("portal", 2) if since == 70 => {
                self.building.on = false;
                self.travel(0);
                next(self, 3);
            }
            ("portal", 3) if settled && since > 60 => {
                // Back to the realm it was built in, the furthest visited, as travel() starts
                // building afresh
                let back = self.session.realms.as_ref().and_then(|r| r.visited.iter().copied().filter(|&r| r > 0).max()).unwrap_or(1) as usize;
                println!("frame {frame}: home in the normal world, going back to {}", REALMS[back].name);
                self.travel(back);
                self.building.back_to = back;
                next(self, 4);
            }
            ("portal", 4) if settled && since > 60 => {
                let feet = self.player.as_ref().map(|p| p.position);
                let places = self.session.places.clone().unwrap_or(found);
                let portal = on_ground(self, places.gate[0] * vpm - ox, places.gate[1] * vpm - oy);
                println!(
                    "frame {frame}: back in {}, standing at {:?} m, the portal home at {:.1}, {:.1} m, {:.1} m off",
                    REALMS[self.building.back_to].name,
                    feet.map(|f| [(f.x + ox) / vpm, (f.y + oy) / vpm]),
                    places.gate[0],
                    places.gate[1],
                    feet.map_or(f64::NAN, |f| (f - portal).xy().norm() / vpm)
                );
                let eye = feet.map_or(portal + front * 5.0 * vpm, |f| f) + up(1.7);
                self.look_from(eye + (eye - portal).normalize() * 3.0 * vpm, portal + up(2.5));
                next(self, 5);
            }
            ("portal", 5) if since == 40 => self.shoot_soon("came back to it"),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save;

    fn found(realm: usize) -> Places {
        Places { realm, arrival: [100.0, 90.0], gate: [100.0, 100.0], facing: -FRAC_PI_2, lairs: [[0.0; 2]; crate::realms::RAIDS], finale: None }
    }

    #[test]
    fn the_gate_stands_once_built_and_moves_with_all_set_in_it() {
        // A new game: no gate, the compass to where it stood
        let mut progress = RealmsSave::default();
        assert_eq!(progress.gate, GateSave::Unbuilt);
        let (places, standing) = stand(&found(0), &progress);
        assert!(!standing);
        assert_eq!(places.gate, [100.0, 100.0]);
        // Built: it stands there, the player coming out in front of it
        progress.placed = vec![3, 1];
        assert!(!build_in(&mut progress, false, [140.0, 120.0], 0.0));
        let (places, standing) = stand(&found(0), &progress);
        assert!(standing);
        assert_eq!(places.gate, [140.0, 120.0]);
        assert!((places.arrival[0] - 144.5).abs() < 1e-9 && (places.arrival[1] - 120.0).abs() < 1e-9);
        // The raid bosses' lairs stay where they were found
        assert_eq!(places.lairs, found(0).lairs);
        // Built again elsewhere, it moves, and what's set into it goes with it
        assert!(build_in(&mut progress, false, [60.0, 70.0], PI));
        let (places, _) = stand(&found(0), &progress);
        assert_eq!(places.gate, [60.0, 70.0]);
        assert_eq!(progress.placed, vec![3, 1]);
        // Through the save and back
        let saved = save::Save { version: save::VERSION, mode: Mode::Realms, realms: Some(progress.clone()), ..Default::default() };
        let back = save::parse(serde_json::to_string(&saved).unwrap().as_bytes(), Mode::Realms).unwrap();
        assert_eq!(back.realms.as_ref().unwrap().gate, progress.gate);
    }

    #[test]
    fn portals_home_are_one_a_realm() {
        let mut progress = RealmsSave { realm: 2, ..Default::default() };
        // The one found stands till one's built
        let (places, standing) = stand(&found(2), &progress);
        assert!(standing && places.gate == [100.0, 100.0]);
        assert!(!build_in(&mut progress, true, [10.0, 20.0], 0.5));
        assert!(build_in(&mut progress, true, [30.0, 40.0], 0.5));
        assert_eq!(progress.portals.len(), 1);
        let (places, _) = stand(&found(2), &progress);
        assert_eq!(places.gate, [30.0, 40.0]);
        // Another realm's own is its own, and the gate's untouched
        progress.realm = 3;
        build_in(&mut progress, true, [1.0, 2.0], 0.0);
        assert_eq!(progress.portals.len(), 2);
        assert_eq!(stand(&found(2), &progress).0.gate, [30.0, 40.0]);
        assert_eq!(progress.gate, GateSave::Unbuilt);
    }

    #[test]
    fn saves_from_before_keep_the_gate_where_it_stood() {
        // A save from before the gate was built by the player has no word of it: it stands where
        // it was found
        let old = r#"{"version":1,"mode":"realms","seed":1,"world":"normal","realms":{"realm":0,"unlocked":[0,1],"visited":[0,1],"placed":[3],"completed":0}}"#;
        let save = save::parse(old.as_bytes(), Mode::Realms).unwrap();
        let progress = save.realms.unwrap();
        assert_eq!(progress.gate, GateSave::Found);
        assert!(progress.portals.is_empty() && !progress.ruins);
        let (places, standing) = stand(&found(0), &progress);
        assert!(standing && places.gate == [100.0, 100.0]);
        // A new game's starts without one, and says so in its save
        let fresh = save::Save { version: save::VERSION, mode: Mode::Realms, realms: Some(RealmsSave::default()), ..Default::default() };
        let back = save::parse(serde_json::to_string(&fresh).unwrap().as_bytes(), Mode::Realms).unwrap();
        assert_eq!(back.realms.unwrap().gate, GateSave::Unbuilt);
    }

    #[test]
    fn the_gate_costs_more_than_a_portal_home() {
        assert!(GATE_COST.stone > PORTAL_COST.stone && GATE_COST.clay > PORTAL_COST.clay && GATE_COST.wood > PORTAL_COST.wood);
        assert_eq!((GATE_COST.wood, GATE_COST.stone, GATE_COST.clay), (20, 60, 30));
    }
}
