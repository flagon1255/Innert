//! The second storm of lightning, see weapons::Power, for every class and none of it a priest's
//! spear: the Stormbrand, a blade raised for lightning to strike it and slashed down to throw the
//! bolt on into what's ahead, left charged a while so every blow calls lightning down; the
//! Stormrunner Slam, runners of lightning racing off along the ground after what's near; the
//! Thundersplit Arrow, splitting into four at the first thing it strikes; the Galvanic Railshot, a
//! dead straight rail of it shot from a gun or crossbow; the Twin Arc, two chakrams thrown wide
//! with lightning strung between them; the Judgment Bolt, a storm gathering over a marked spot and
//! one great bolt coming down out of it; the Galvanic Torrent, lightning poured out of the hand
//! while right click's held; and Grave Lightning, breaking up out of the ground round where it's
//! cast, giving back to the caster a little of the life of what it strikes.
//!
//! Every bolt jagged and forked, see lightning.rs, white-hot enough to be seen by day, each let go
//! with a move of its own and leaving the weapon as something flying, thunder as big as it is, and
//! harder on what stands in water. What they shoot and leave going is powers.rs's machinery, shapes
//! and lastings of their own.

use super::powers::{Lasting, Shape};
use super::*;
use crate::weapons::Power;

/// The Stormbrand: how far its arcs leap from the blade, in meters, to how many, when in its move
/// the lightning strikes the raised blade and when it's slashed down, as parts of it, how long the
/// blade stays charged after, in seconds, and how hard each blow's lightning strikes next to the
/// blow
pub(in crate::combat) const BRAND_REACH: f64 = 14.0;
const BRAND_ARCS: usize = 3;
const BRAND_STRUCK: f64 = 0.3;
const BRAND_RELEASE: f64 = 0.56;
pub(in crate::combat) const BRAND_SECONDS: f64 = 8.0;
const BRAND_BLOW: f64 = 0.4;
/// The Stormrunner Slam: how far its runners run, in meters, how many, how fast, in meters a
/// second, and how high over the ground
pub(in crate::combat) const RUNNER_REACH: f64 = 16.0;
/// How far round the ring the runners look for what to turn after, in meters
pub(in crate::combat) const RUNNER_ROUND: f64 = 6.0;
const RUNNERS: usize = 5;
const RUNNER_SPEED: f64 = 13.0;
const RUNNER_HIGH: f64 = 0.35;
/// The Thundersplit Arrow: how fast it flies, how many it splits into, how far they look for what
/// to leap on to, in meters, and how fast they fly
const SPLIT_SPEED: f64 = 55.0;
const SPLITS: usize = 4;
const SPLIT_LEAP: f64 = 12.0;
const SPLINTER_SPEED: f64 = 36.0;
/// The Galvanic Railshot: how far its rail reaches, in meters, how fast its slug flies, and how
/// long the rail hangs, in seconds
pub(in crate::combat) const RAIL_REACH: f64 = 45.0;
const RAIL_SPEED: f64 = 160.0;
const RAIL_SECONDS: f64 = 0.6;
/// The Twin Arc: how far out its chakrams go, how fast, in meters and meters a second, how far
/// either side of their middle they spread at the widest, and how near the line between them
/// counts as crossed, in meters
pub(in crate::combat) const TWIN_REACH: f64 = 15.0;
const TWIN_SPEED: f64 = 20.0;
const TWIN_SPREAD: f64 = 2.4;
const TWIN_CUT: f64 = 0.8;
/// The Judgment Bolt: how far round it strikes, and hardest, in meters, how long the storm gathers
/// before it does, in seconds, and how high over the ring it gathers, in meters
pub(in crate::combat) const JUDGE_ROUND: f64 = 5.0;
const JUDGE_CORE: f64 = 2.5;
const JUDGE_GATHER: f64 = 0.9;
const JUDGE_HEIGHT: f64 = 7.5;
/// The Galvanic Torrent: how far it pours and how wide, in meters and radians either side, how
/// long it can be held, how often a spark of it goes, in seconds, how fast, in meters a second,
/// how hard each spark and each spark of the last surge strike next to a spell, and how many
/// times a spell's mana it drains a second
pub(in crate::combat) const TORRENT_REACH: f64 = 11.0;
pub(in crate::combat) const TORRENT_FAN: [f64; 3] = [-0.35, 0.0, 0.35];
pub(in crate::combat) const TORRENT_MOST: f64 = 3.0;
const TORRENT_EVERY: f64 = 0.07;
const TORRENT_SPEED: f64 = 48.0;
const TORRENT_BITE: f64 = 0.11;
const TORRENT_SURGE: f64 = 0.5;
const TORRENT_DRAIN: f64 = 1.0;
/// Where a weapon pouring a torrent is held, off its rest and pointing, as spell_at() has it
const TORRENT_HELD: ([f64; 3], [f64; 3]) = ([-0.05, 0.18, -0.02], [-0.07, 0.95, 0.3]);
/// Grave Lightning: how far round it breaks up, how many times, how long apart, how near a strike
/// has to be to hurt, in meters and seconds, and how much of what it does comes back as health
pub(in crate::combat) const GRAVE_ROUND: f64 = 4.0;
const GRAVE_STRIKES: usize = 8;
const GRAVE_GAP: f64 = 0.13;
const GRAVE_HIT: f64 = 1.7;
const GRAVE_DRAIN: f64 = 0.3;

/// How long their own moves take, and when what they let go leaves the weapon
pub(in crate::combat) fn thunder_timing(power: Power) -> Option<(f64, f64)> {
    Some(match power {
        Power::Stormbrand => (1.1, BRAND_RELEASE),
        Power::Stormrunners => (1.05, 0.55),
        Power::Thundersplit => (0.85, 0.5),
        Power::Railshot => (0.95, 0.5),
        Power::TwinArc => (0.8, 0.42),
        Power::Judgment => (1.0, 0.5),
        Power::Torrent => (0.6, 0.22),
        Power::Gravebolt => (1.0, 0.52),
        _ => return None,
    })
}

/// Their own moves, from where the weapon's held at rest
pub(in crate::combat) fn thunder_keys(power: Power, profile: &Profile) -> Option<Vec<(f64, Pose, bool)>> {
    let (_, release) = thunder_timing(power)?;
    let at = spell_at(profile.style);
    let magic = profile.kind == Kind::Magic && profile.style != Style::Glaive;
    Some(match power {
        // Dipped, raised up at the sky and held there trembling as the lightning strikes it, then
        // wound back out to the right and slashed across, the bolt leaping on off it, and carried
        // on round to the left
        Power::Stormbrand => {
            let up = [0.0, 0.92, 0.4];
            let flat = [0.0, 0.0, 1.0];
            vec![
                (0.08, pose([0.3, 0.42, -0.36], [0.05, 0.85, -0.5], [0.0, 0.5, 0.85]), false),
                (BRAND_STRUCK - 0.08, pose([0.18, 0.48, -0.16], [0.02, 0.66, 0.75], up), false),
                (BRAND_STRUCK, pose([0.18, 0.5, -0.14], [0.02, 0.7, 0.71], up), false),
                (0.4, pose([0.185, 0.49, -0.135], [0.04, 0.68, 0.73], up), false),
                (release - 0.08, pose([0.44, 0.36, -0.1], [0.85, 0.3, 0.42], flat), false),
                (release, pose([0.0, 0.68, -0.2], [0.05, 0.99, 0.05], flat), true),
                (release + 0.12, pose([-0.42, 0.44, -0.28], [-0.9, 0.32, 0.1], flat), false),
                (0.88, pose([-0.1, 0.5, -0.3], [-0.3, 0.55, 0.78], [1.0, 0.0, 0.0]), false),
            ]
        }
        // Dipped, raised up before the face, its head at the top of the view for the lightning to
        // be seen gathering in it, held there trembling, drawn back over and slammed down onto the
        // ground in front
        Power::Stormrunners => {
            let up = [0.0, 0.92, 0.4];
            let slammed = pose([0.05, 0.62, -0.42], [0.0, 0.62, -0.78], [0.0, -0.78, -0.62]);
            let mut settled = slammed.clone();
            settled.hand.z -= 0.04;
            vec![
                (0.08, pose([0.3, 0.42, -0.36], [0.05, 0.85, -0.5], [0.0, 0.5, 0.85]), false),
                (0.2, pose([0.18, 0.48, -0.16], [0.02, 0.66, 0.75], up), false),
                (0.3, pose([0.18, 0.5, -0.14], [0.02, 0.7, 0.71], up), false),
                (0.42, pose([0.185, 0.49, -0.135], [0.04, 0.68, 0.73], up), false),
                (release - 0.07, pose([0.16, 0.28, 0.1], [0.05, -0.4, 0.92], up), false),
                (release, slammed, true),
                (0.76, settled, false),
            ]
        }
        // Raised, brought down onto the crosshair, drawn a long moment as the arrow crackles, and
        // loosed with a snap
        Power::Thundersplit => vec![
            (0.16, bow_aimed(0.14, 0.0), false),
            (0.32, bow_held(0.06, 0.0), false),
            (release - 0.05, bow_held(0.05, 0.0), false),
            (release, bow_loosed(0.05, 0.0), true),
            (0.82, bow_aimed(0.02, 0.0), false),
        ],
        // Shouldered and held steady as the lightning winds round the muzzle, then kicking back
        // hard as the rail goes
        Power::Railshot => {
            let aimed = levelled(0.03, 0.0);
            let mut lowered = levelled(0.02, 0.0);
            lowered.hand.z -= 0.02;
            vec![
                (0.12, levelled(0.12, 0.1), false),
                (0.24, aimed.clone(), false),
                (release - 0.1, steadied(&aimed, 1.0), false),
                (release - 0.02, steadied(&aimed, 1.5), false),
                (release, gun_kicked(&aimed, 1.6), true),
                (release + 0.14, rocked(&aimed, 1.6), false),
                (0.85, lowered, false),
            ]
        }
        // Drawn back over the shoulder, held, and hurled overhand
        Power::TwinArc => vec![
            (0.18, pose([0.3, 0.28, 0.0], [0.1, -0.4, 0.91], [1.0, 0.0, 0.0]), false),
            (release - 0.06, pose([0.33, 0.16, 0.08], [0.08, -0.66, 0.75], [1.0, 0.0, 0.0]), false),
            (release, pose([0.14, 0.72, -0.06], [0.0, 0.92, 0.38], [1.0, 0.0, 0.0]), true),
            (release + 0.16, pose([0.1, 0.62, -0.32], [0.0, 0.6, -0.8], [1.0, 0.0, 0.0]), false),
            (0.88, pose([0.26, 0.46, -0.27], [0.1, 0.25, 0.96], [1.0, 0.0, 0.0]), false),
        ],
        // Raised high and held trembling as the mote gathers in it, then thrust up at the sky over
        // the ring as it goes, and lowered to point at it
        Power::Judgment => vec![
            (0.12, at([0.0, -0.05, 0.12], [0.0, -0.3, 0.95]), false),
            (0.3, at([0.01, -0.07, 0.17], [0.02, -0.42, 0.9]), false),
            (release - 0.05, at([0.0, -0.06, 0.18], [0.0, -0.4, 0.92]), false),
            (release, at([-0.04, 0.12, 0.12], [-0.04, 0.55, 0.84]), true),
            (0.66, at([-0.05, 0.15, 0.02], [-0.05, 0.85, 0.5]), false),
            (0.88, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
        ],
        // Held out at the crosshair as it was while it poured, drawn in a little, then thrust out
        // hard as the last surge breaks out of it
        Power::Torrent => vec![
            (0.05, at(TORRENT_HELD.0, TORRENT_HELD.1), false),
            (release - 0.04, at([-0.02, 0.1, 0.02], [-0.02, 0.75, 0.66]), false),
            (release, at([-0.07, 0.24, -0.03], [-0.09, 0.97, 0.2]), true),
            (release + 0.14, at([-0.05, 0.2, -0.02], [-0.06, 0.94, 0.33]), false),
            (0.86, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
        ],
        // Raised and held as the dead ground's called on, then driven down at the ground ahead
        Power::Gravebolt if magic => vec![
            (0.15, at([0.0, -0.05, 0.12], [0.0, -0.25, 0.97]), false),
            (0.36, at([0.01, -0.07, 0.17], [0.02, -0.35, 0.94]), false),
            (release - 0.05, at([0.0, -0.06, 0.18], [0.0, -0.3, 0.95]), false),
            (release, at([-0.03, 0.22, -0.1], [-0.04, 0.7, -0.71]), true),
            (release + 0.14, at([-0.03, 0.23, -0.13], [-0.04, 0.64, -0.77]), false),
            (0.86, at([-0.01, 0.03, 0.0], [-0.02, 0.4, 0.92]), false),
        ],
        // A scythe's: raised high in both hands, then its blade driven down into the ground ahead
        Power::Gravebolt => vec![
            (0.2, pose([0.18, 0.3, 0.04], [0.1, 0.05, 1.0], [0.0, 1.0, -0.05]), false),
            (release - 0.06, pose([0.18, 0.27, 0.1], [0.1, -0.12, 0.99], [0.0, 0.99, 0.12]), false),
            (release, pose([0.04, 0.6, -0.38], [0.0, 0.62, -0.78], [0.0, -0.78, -0.62]), true),
            (release + 0.12, pose([0.04, 0.62, -0.44], [0.0, 0.52, -0.85], [0.0, -0.85, -0.52]), false),
            (0.9, pose([0.2, 0.48, -0.3], [0.0, 0.55, 0.83], [0.0, 1.0, -0.1]), false),
        ],
        _ => return None,
    })
}

/// Where a Twin Arc's two chakrams are, `p` being their middle, out `reach` meters and `back` or
/// not: together as they leave the hand, spread wide halfway out, crossing at the far end, spread
/// again halfway back and together again as they come back to it
fn twin_ends(p: &Projectile, back: bool, reach: u8, vpm: f64) -> (Vector3<f64>, Vector3<f64>) {
    let out = (reach as f64).max(1.0) * vpm;
    let g = if back { 1.0 + (p.flown - out).max(0.0) / out } else { p.flown / out };
    let spread = (TWIN_SPREAD * (PI * g.min(2.0)).sin().abs()).max(0.3);
    let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
    let across = way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x);
    (p.position + across * spread * vpm, p.position - across * spread * vpm)
}

/// How far `at` is from the line from `a` to `b`, in voxels
fn off_segment(at: Vector3<f64>, a: Vector3<f64>, b: Vector3<f64>) -> f64 {
    let ab = b - a;
    let t = ((at - a).dot(&ab) / ab.norm_squared().max(1e-9)).clamp(0.0, 1.0);
    (at - (a + ab * t)).norm()
}

impl Combat {
    /// One of them let go: true if it was one of them
    pub(in crate::combat) fn unleash_thunder(&mut self, arena: &mut Arena, weapon: &Weapon, power: Power, base: f64) -> bool {
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let storm_profile = Rc::new(Profile { element: Some(Element::Storm), ..(*weapon.profile).clone() });
        let profile = &storm_profile;
        let storm = Element::look(Some(Element::Storm));
        let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
        let forward = view.forward();
        let flat = flat_of(forward);
        let light = Element::Storm.light();
        match power {
            // The bolt the blade caught thrown on off it into what the crosshair's on and what's
            // near that, or forked down into the ground ahead, and the blade left charged
            Power::Stormbrand => {
                let tip = self.held_point(weapon, view, false);
                let (targets, spot) = Combat::seek_targets(arena, BRAND_REACH);
                let mut struck = 0;
                for &i in targets.iter().take(BRAND_ARCS) {
                    let center = arena.animals[i].center(vpm as u32);
                    self.landed(center);
                    self.lightning(tip, center, storm, 0.08, 3, 3, 0.0, if struck == 0 { 1.4 } else { 0.0 });
                    let damage = base * 1.2 * self.wet(arena, i);
                    self.power_hit(arena, profile, i, center, damage, tip);
                    if arena.animals[i].health > 0.0 {
                        arena.animals[i].status.shocked = arena.animals[i].status.shocked.max(1.0);
                    }
                    struck += 1;
                }
                // With nothing there, forked out onto the ring the crosshair's on and round it, the
                // ground scorched where that's on the ground
                if struck == 0 {
                    let across = flat.cross(&up);
                    for k in 0..3 {
                        let to = spot + across * (k as f64 - 1.0) * 1.2 * vpm;
                        self.landed(to);
                        self.lightning(tip, to, storm, 0.07, 2, 2, 0.02 * k as f64, if k == 1 { 1.2 } else { 0.0 });
                        self.power_pop(arena, profile, to, 1.5, base * 0.4, None);
                        let ground = Combat::floor_at(arena, to);
                        if (ground - to).norm() < 1.0 * vpm {
                            self.scorch(ground, 0.7);
                        }
                    }
                }
                self.charged_blade = BRAND_SECONDS;
                self.flashes.push(Flash { position: tip, color: light, reach: 8.0, life: 0.25, lifetime: 0.25 });
                self.sparks(tip, 16, 4.5, storm, 0.5, 0.4);
                self.cues.push(Cue::here(Sound::Thunder, 0.9).pitched(0.95));
                self.cues.push(Cue::here(Sound::Shing, 0.6).pitched(1.3));
                self.shake = self.shake.max(0.35);
            }
            // Runners of lightning sent racing off along the ground in a fan from where it came
            // down, each turning after what the marks were over
            Power::Stormrunners => {
                // Out over the ground to the ring it was aimed at, each turning after one of what's
                // round it, or with nothing there spreading out across it
                let target = self.aim(arena);
                let spot = self.power_spot(arena, weapon).unwrap_or(target);
                let head = ground_under(arena.world, feet + flat * 1.6 * vpm + up * vpm, vpm);
                let targets = Combat::foes_round(arena, spot, RUNNER_ROUND);
                let across = flat_of(spot - head).cross(&up);
                for k in 0..RUNNERS {
                    let s = (k as f64 - (RUNNERS - 1) as f64 * 0.5) / ((RUNNERS - 1) as f64 * 0.5);
                    let goal = spot + across * s * 1.2 * vpm;
                    let (way, range) = match targets.get(k % targets.len().max(1)) {
                        Some(&i) => {
                            let fan = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), s * 0.75) * flat;
                            let off = arena.animals[i].position - head;
                            let to = flat_of(off);
                            ((fan + to * 1.5).try_normalize(1e-6).unwrap_or(fan), (off.xy().norm() / vpm + 1.0).min(RUNNER_REACH))
                        }
                        None => (flat_of(goal - head), ((goal - head).xy().norm() / vpm).clamp(2.0, RUNNER_REACH)),
                    };
                    let start = head + way * 0.4 * vpm + up * RUNNER_HIGH * vpm;
                    self.launch(profile, None, Form::Bolt, start, way * RUNNER_SPEED * vpm, base * 0.9, 0.0);
                    self.signed(Sig::Power(Shape::Runner));
                    if let Some(p) = self.projectiles.last_mut() {
                        p.range = (range - 0.4) * vpm;
                        p.seek = if targets.is_empty() { 0.0 } else { 3.5 };
                    }
                }
                self.keep_power_ring(weapon, spot, (spot - head).norm() / vpm / RUNNER_SPEED + 0.2);
                self.blasts.push(Blast { position: head + up * 0.2 * vpm, look: storm, radius: 0.6, life: 0.25, lifetime: 0.25 });
                // Forks of it cracking out ahead over the ground from where it came down, none back
                // at the eye
                for k in 0..3 {
                    let way = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), (k as f64 - 1.0) * 0.6) * flat;
                    let to = ground_under(arena.world, head + way * 2.2 * vpm + up * vpm, vpm);
                    self.lightning(head + up * 0.15 * vpm, to + up * 0.1 * vpm, storm, 0.035, 1, 2, 0.0, if k == 1 { 1.2 } else { 0.0 });
                }
                self.sparks(head, 18, 5.0, storm, 0.8, 0.4);
                self.scorch(head, 1.0);
                self.cues.push(Cue::here(Sound::Slam, 0.9).pitched(0.8));
                self.cues.push(Cue::here(Sound::Thunder, 0.8).pitched(1.15));
                self.shake = self.shake.max(0.5);
            }
            // An arrow of lightning from the nock at the crosshair
            Power::Thundersplit => {
                let from = self.held_point(weapon, view, true);
                let target = self.aim(arena);
                let way = lead(from, target, SPLIT_SPEED, 0.2, vpm);
                self.launch(profile, Some(Missile { shot: Shot::Arrow, head: weapon.arrowhead(), power: true, size: 1.5 }), Form::Bolt, from, way * SPLIT_SPEED * vpm, base * 1.8, 0.2);
                self.signed(Sig::Power(Shape::Volt { split: true }));
                if let Some(p) = self.projectiles.last_mut() {
                    p.size = 1.5;
                }
                self.lightning(from, from + way * 1.4 * vpm, storm, 0.03, 1, 2, 0.0, 0.0);
                self.flashes.push(Flash { position: from, color: light, reach: 6.0, life: 0.25, lifetime: 0.25 });
                self.cues.push(Cue::here(Sound::Twang, 1.0).pitched(0.8));
                self.cues.push(Cue::here(Sound::Zap, 0.8).pitched(0.75));
            }
            // A rail of lightning, dead straight, from the muzzle to the first wall or as far as it
            // reaches, a slug down it through everything in its way
            Power::Railshot => {
                let from = self.held_point(weapon, view, false);
                let (to, _) = self.power_points(arena, power)[0];
                let way = (to - from).try_normalize(1e-6).unwrap_or(forward);
                let far = from + way * RAIL_REACH * vpm;
                let end = from + (far - from) * solid_along(arena.world, from, far).unwrap_or(1.0);
                let length = (end - from).norm();
                self.launch(profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.4 }), Form::Bolt, from, way * RAIL_SPEED * vpm, base * 2.2, 0.0);
                self.signed(Sig::Power(Shape::Rail));
                if let Some(p) = self.projectiles.last_mut() {
                    p.range = length + 0.3 * vpm;
                    p.size = 1.4;
                }
                let start = from + way * 0.8 * vpm;
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Rail)), profile, start, end - start, 1.0, RAIL_SECONDS, 0.0);
                // Lightning wound along the rail its whole length, a stretch of it at a time
                let pieces = ((length / vpm / 4.0).ceil() as usize).clamp(1, 6);
                for k in 0..pieces {
                    let a = start + (end - start) * (k as f64 / pieces as f64);
                    let b = start + (end - start) * ((k + 1) as f64 / pieces as f64);
                    self.lightning(a, b, storm, 0.06, 1, 3, 0.0, if k == 0 { 1.2 } else { 0.0 });
                }
                self.flashes.push(Flash { position: start + (end - start) * 0.3, color: light, reach: 10.0, life: 0.3, lifetime: 0.3 });
                if weapon.profile.style == Style::Powder {
                    self.gun_report(weapon, view, &weapon.profile, from, way, 1.4);
                } else {
                    self.cues.push(Cue::here(Sound::Release, 1.0).pitched(0.7));
                    self.kick += 0.05;
                }
                self.cues.push(Cue::here(Sound::Thunder, 0.7).pitched(1.6));
                self.cues.push(Cue::here(Sound::Zap, 1.0).pitched(0.6));
                self.shake = self.shake.max(0.3);
            }
            // Two chakrams hurled out spreading apart, lightning strung between them, at what the
            // crosshair first meets, and back
            Power::TwinArc => {
                let from = view.eye + view.rotation * Vector3::new(0.1, 1.0, -0.2) * vpm;
                let (to, who) = self.power_points(arena, power)[0];
                let way = (to - from).try_normalize(1e-6).unwrap_or(flat);
                let reach = ((to - from).norm() / vpm + if who.is_some() { 2.5 } else { 0.0 }).clamp(6.0, TWIN_REACH);
                self.launch(profile, None, Form::Bolt, from, way * TWIN_SPEED * vpm, base * 0.9, 0.0);
                self.signed(Sig::Power(Shape::Twin { back: false, reach: reach.round() as u8 }));
                if who.is_none() {
                    self.keep_power_marks(power, &[to], reach / TWIN_SPEED);
                }
                self.lightning(from - view.rotation * Vector3::x() * 0.2 * vpm, from + view.rotation * Vector3::x() * 0.2 * vpm, storm, 0.03, 0, 2, 0.0, 0.0);
                self.cues.push(Cue::here(Sound::Whirr, 0.9).pitched(1.1));
                self.cues.push(Cue::here(Sound::Zap, 0.7).pitched(1.0));
            }
            // A mote sent up over the ring, the storm gathering there, see judgment()
            Power::Judgment => {
                let from = self.held_point(weapon, view, false);
                let target = self.aim(arena);
                let spot = self.power_spot(arena, weapon).unwrap_or(target);
                self.keep_power_ring(weapon, spot, JUDGE_GATHER + 0.1);
                let top = spot + up * JUDGE_HEIGHT * vpm;
                let time = 0.45;
                self.launch(profile, None, Form::Bolt, from, (top - from) / time, 0.0, 0.0);
                self.signed(Sig::Power(Shape::Summon));
                if let Some(p) = self.projectiles.last_mut() {
                    p.range = (top - from).norm();
                    p.size = 1.2;
                }
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Judgment)), profile, spot, Vector3::zeros(), JUDGE_ROUND, JUDGE_GATHER + 0.5, base * 3.2);
                self.flashes.push(Flash { position: from, color: light, reach: 6.0, life: 0.25, lifetime: 0.25 });
                self.cues.push(Cue::here(Sound::Summon, 0.8).pitched(0.6));
                self.cues.push(Cue::at(Sound::Rumble, top, 0.8).pitched(0.7));
            }
            // The last surge as it's let go, six at once
            Power::Torrent => {
                let from = self.held_point(weapon, view, false);
                for _ in 0..6 {
                    self.torrent_spark(arena, weapon, from, true);
                }
                self.flashes.push(Flash { position: from, color: light, reach: 7.0, life: 0.25, lifetime: 0.25 });
                self.cues.push(Cue::here(Sound::Thunder, 0.55).pitched(1.3));
                self.poured = 0.0;
            }
            // A mote of storm cast onto the ring, the ground breaking up in lightning where it
            // comes down, see grave_breaks()
            Power::Gravebolt => {
                let from = self.held_point(weapon, view, false);
                let target = self.aim(arena);
                let spot = self.power_spot(arena, weapon).unwrap_or(target);
                let time = ((spot - from).norm() / (16.0 * vpm)).max(0.3);
                self.keep_power_ring(weapon, spot, time + 0.3);
                self.launch(profile, None, Form::Bolt, from, thrown(from, spot, time, 0.4, vpm), base * 0.8, 0.4);
                self.signed(Sig::Power(Shape::Grave));
                self.cues.push(Cue::here(Sound::Hex, 0.7).pitched(0.6));
                self.cues.push(Cue::here(Sound::Zap, 0.5).pitched(0.6));
            }
            _ => return false,
        }
        true
    }

    /// The weapon through one of their moves, `f` of the way through it: lightning striking the
    /// raised blade and crackling on it, gathering in a hammer's head, round a muzzle, at a nock
    /// or in the hand, the dead ground giving up motes of storm
    pub(in crate::combat) fn thunder_gathers(&mut self, arena: &Arena, weapon: &Weapon, power: Power, f: f64, dt: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let storm = Element::look(Some(Element::Storm));
        let Some((length, release)) = thunder_timing(power) else {
            return;
        };
        let step = dt / length;
        let gathering = f > 0.08 && f < release;
        let tip = self.held_point(weapon, view, power == Power::Thundersplit);
        match power {
            // Struck by lightning out of the sky as it's held up, and crackling till it's swung
            Power::Stormbrand => {
                if f < BRAND_STRUCK && f + step >= BRAND_STRUCK {
                    // Out of the sky ahead, so it's seen coming down across it into the blade
                    let ahead = flat_of(view.forward()) * 6.0 * vpm;
                    let lean = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, 0.0) * 2.0 * vpm;
                    self.lightning(tip + up * 9.0 * vpm + ahead + lean, tip, storm, 0.11, 3, 3, 0.0, 1.6);
                    self.sparks(tip, 14, 4.0, storm, 0.6, 0.35);
                    self.cues.push(Cue::here(Sound::Thunder, 0.8).pitched(1.1));
                    self.cues.push(Cue::here(Sound::Crackle, 0.7).pitched(1.2));
                    self.shake = self.shake.max(0.25);
                }
                if f >= BRAND_STRUCK && f < release + 0.1 {
                    self.blade_crackles(arena, weapon, dt, 1.0);
                }
            }
            // Gathering crackling in the head held high, arcs of it reaching down for the ground
            Power::Stormrunners | Power::Gravebolt if gathering => {
                let n = (40.0 * dt * f + random(&mut self.rng)) as usize;
                for _ in 0..n {
                    let d = self.any_direction();
                    let at = tip + d * (0.35 + 0.3 * random(&mut self.rng)) * vpm;
                    self.fine(at, -d * 1.6 * vpm, storm, 0.22, 0.0);
                }
                if random(&mut self.rng) < dt * 7.0 {
                    let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
                    let a = random(&mut self.rng) * TAU;
                    let to = ground_under(arena.world, feet + Vector3::new(a.cos(), a.sin(), 0.0) * (1.2 + random(&mut self.rng)) * vpm + up * vpm, vpm);
                    self.lightning(tip, to, storm, 0.025, 1, 1, 0.0, 0.0);
                    self.cues.push(Cue::here(Sound::Zap, 0.3).pitched(1.2 + f as f32));
                }
                // The dead ground giving up motes of shadow to it
                if power == Power::Gravebolt && random(&mut self.rng) < dt * 25.0 {
                    let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
                    let a = random(&mut self.rng) * TAU;
                    let at = feet + Vector3::new(a.cos(), a.sin(), 0.1) * (0.8 + 1.5 * random(&mut self.rng)) * vpm;
                    self.mote(at, Some(Element::Void), 1.3);
                }
            }
            // Coils of lightning winding round the muzzle, faster and higher as it charges
            Power::Railshot if gathering => {
                let charge = f / release;
                let n = ((30.0 + 60.0 * charge) * dt + random(&mut self.rng)) as usize;
                for _ in 0..n {
                    let d = self.any_direction();
                    self.fine(tip + d * 0.3 * vpm, -d * 1.2 * vpm + view.forward() * 0.6 * vpm, storm, 0.18, 0.0);
                }
                if random(&mut self.rng) < dt * (6.0 + 14.0 * charge) {
                    let to = tip + self.any_direction() * (0.2 + 0.15 * charge) * vpm;
                    self.lightning(tip, to, storm, 0.015 + 0.01 * charge, 0, 1, 0.0, 0.0);
                    self.cues.push(Cue::here(Sound::Zap, 0.25 + 0.2 * charge as f32).pitched(0.9 + 1.2 * charge as f32));
                }
            }
            // Crackling at the nock, the arrow drawn back alight
            Power::Thundersplit | Power::TwinArc | Power::Torrent if gathering => {
                if random(&mut self.rng) < dt * 30.0 {
                    let d = self.any_direction();
                    self.fine(tip + d * 0.15 * vpm, d * 1.0 * vpm, storm, 0.15, 0.0);
                }
                if random(&mut self.rng) < dt * 9.0 {
                    let to = tip + self.any_direction() * 0.3 * vpm;
                    self.lightning(tip, to, storm, 0.015, 0, 1, 0.0, 0.0);
                }
            }
            // Drawn into the weapon held high, a bolt reaching up out of it for the sky now and then
            Power::Judgment if gathering => {
                let n = ((25.0 + 45.0 * f) * dt + random(&mut self.rng)) as usize;
                for _ in 0..n {
                    let d = self.any_direction();
                    let at = tip + d * (0.4 + 0.3 * random(&mut self.rng)) * vpm;
                    self.fine(at, -d * 1.8 * vpm, storm, 0.22, 0.0);
                }
                if random(&mut self.rng) < dt * 5.0 {
                    let to = tip + (up * 2.5 + self.any_direction()) * vpm;
                    self.lightning(tip, to, storm, 0.03, 1, 1, 0.0, 0.0);
                    self.cues.push(Cue::here(Sound::Zap, 0.35).pitched(0.8));
                }
            }
            _ => {}
        }
    }

    /// Lightning crackling along the blade in hand, `strength` 1 at its fiercest
    fn blade_crackles(&mut self, arena: &Arena, weapon: &Weapon, dt: f64, strength: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let storm = Element::look(Some(Element::Storm));
        let tip = self.held_point(weapon, view, false);
        let hand = view.eye + view.rotation * self.pose(weapon).hand * vpm;
        let along = |s: f64| hand + (tip - hand) * (0.25 + 0.75 * s);
        if random(&mut self.rng) < dt * 40.0 * strength {
            let at = along(random(&mut self.rng));
            let v = self.any_direction() * 1.2 * vpm;
            self.fine(at, v, storm, 0.15, 0.0);
        }
        if random(&mut self.rng) < dt * 9.0 * strength {
            let s = random(&mut self.rng);
            let from = along(s * 0.6);
            let to = along((s + 0.4).min(1.0)) + self.any_direction() * 0.08 * vpm;
            self.lightning(from, to, storm, 0.018, 0, 1, 0.0, 0.0);
            self.flashes.push(Flash { position: tip, color: Element::Storm.light(), reach: 2.5, life: 0.08, lifetime: 0.08 });
        }
        if random(&mut self.rng) < dt * 2.5 * strength {
            self.cues.push(Cue::here(Sound::Zap, 0.15).pitched(1.4 + 0.4 * random(&mut self.rng) as f32));
        }
    }

    /// The blade the Stormbrand charged, as the frames go by while it's in hand, `power` its power:
    /// crackling till its charge runs out, and none on another weapon
    pub(in crate::combat) fn stormbrand_crackles(&mut self, arena: &Arena, weapon: &Weapon, power: Power, dt: f64) {
        if self.charged_blade <= 0.0 {
            return;
        }
        if power != Power::Stormbrand {
            self.charged_blade = 0.0;
            return;
        }
        self.charged_blade -= dt;
        if !matches!(self.action, Action::Power { .. }) {
            let fading = (self.charged_blade / 1.0).clamp(0.3, 1.0);
            self.blade_crackles(arena, weapon, dt, 0.55 * fading);
        }
        if self.charged_blade <= 0.0 {
            self.cues.push(Cue::here(Sound::Zap, 0.3).pitched(0.6));
        }
    }

    /// A blow of a blade the Stormbrand charged landing on animal `i` for `base`: lightning out of
    /// the sky onto it, and on to the next nearest
    pub(in crate::combat) fn stormbrand_strikes(&mut self, arena: &mut Arena, profile: &Rc<Profile>, i: usize, base: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let storm = Element::look(Some(Element::Storm));
        let storm_profile = Rc::new(Profile { element: Some(Element::Storm), ..(**profile).clone() });
        let center = arena.animals[i].center(vpm as u32);
        let lean = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, 0.0) * 1.5 * vpm;
        self.lightning(center + up * 9.0 * vpm + lean, center, storm, 0.08, 2, 2, 0.0, 1.0);
        let damage = base * BRAND_BLOW * self.wet(arena, i);
        self.strike(arena, &storm_profile, i, Blow { damage, crit: false, heavy: false, from: center + up * vpm, depth: 1, element: Some(Element::Storm) });
        if arena.animals[i].health > 0.0 {
            arena.animals[i].status.shocked = arena.animals[i].status.shocked.max(0.6);
        }
        let id = arena.animals[i].id;
        if let Some(j) = Combat::foes_round(arena, center, 5.0).into_iter().find(|&j| arena.animals[j].id != id) {
            let to = arena.animals[j].center(vpm as u32);
            self.lightning(center, to, storm, 0.045, 1, 2, HOP_GAP, 0.0);
            let damage = base * BRAND_BLOW * 0.5 * self.wet(arena, j);
            self.strike(arena, &storm_profile, j, Blow { damage, crit: false, heavy: false, from: center, depth: 1, element: Some(Element::Storm) });
            if arena.animals[j].health > 0.0 {
                arena.animals[j].status.shocked = arena.animals[j].status.shocked.max(0.5);
            }
        }
        self.sparks(center, 10, 3.5, storm, 0.6, 0.3);
        self.cues.push(Cue::at(Sound::Thunder, center, 0.45).pitched(1.35));
        self.cues.push(Cue::at(Sound::Crackle, center, 0.6).pitched(1.1));
    }

    /// A Galvanic Torrent pouring out of the weapon in hand while right click's held: a spark of
    /// it every TORRENT_EVERY, the mana draining as it goes, sputtering once there's only enough
    /// left for its last surge
    pub(in crate::combat) fn torrent_pours(&mut self, arena: &Arena, weapon: &Weapon, dt: f64) {
        let profile = &weapon.profile;
        let from = self.held_point(weapon, arena.view, false);
        let keep = profile.stat("manaCost") * Power::Torrent.mana();
        let drain = profile.stat("manaCost") * TORRENT_DRAIN * dt;
        if self.mana - drain < keep {
            if random(&mut self.rng) < dt * 10.0 {
                let v = self.any_direction() * 0.8 * arena.view.vpm;
                self.fine(from, v, Element::look(Some(Element::Storm)), 0.15, 0.3);
            }
            return;
        }
        self.mana -= drain;
        self.poured += dt;
        while self.poured >= TORRENT_EVERY {
            self.poured -= TORRENT_EVERY;
            self.torrent_spark(arena, weapon, from, false);
        }
    }

    /// The weapon in hand held out at the crosshair as a torrent pours out of it, shaking with it,
    /// the more the longer it's poured
    pub(in crate::combat) fn torrent_pose(&self, weapon: &Weapon) -> Pose {
        let mut p = spell_at(weapon.profile.style)(TORRENT_HELD.0, TORRENT_HELD.1);
        let shake = 0.003 + 0.004 * (self.power.aimed / TORRENT_MOST).min(1.0);
        p.hand.x += shake * (self.time * 53.0).sin();
        p.hand.z += shake * (self.time * 41.0).cos();
        p
    }

    /// One spark of a torrent from `from`, or with `surge` one of its last surge: at one of the
    /// nearest few ahead, turning after it, or anywhere in its fan
    fn torrent_spark(&mut self, arena: &Arena, weapon: &Weapon, from: Vector3<f64>, surge: bool) {
        let view = arena.view;
        let vpm = view.vpm;
        let storm = Element::look(Some(Element::Storm));
        let profile = Rc::new(Profile { element: Some(Element::Storm), ..(*weapon.profile).clone() });
        let base = profile.stat("spellDamage") + profile.stat("elementalDamage");
        let ahead = Combat::foes_ahead(arena, view.eye, view.forward(), TORRENT_REACH, TORRENT_FAN[2].cos());
        let pick = (random(&mut self.rng) * ahead.len().min(3) as f64) as usize;
        let (to, seek) = match ahead.get(pick) {
            Some(&i) => (arena.animals[i].center(vpm as u32), 9.0),
            None => {
                let yaw = (random(&mut self.rng) - 0.5) * 2.0 * TORRENT_FAN[2];
                let way = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), yaw) * view.forward() + Vector3::new(0.0, 0.0, (random(&mut self.rng) - 0.5) * 0.15);
                (Combat::ray_point(arena, view.eye, way.normalize(), TORRENT_REACH).0, 0.0)
            }
        };
        let way = (to - from).try_normalize(1e-6).unwrap_or_else(|| view.forward());
        let along = (way + self.any_direction() * if surge { 0.18 } else { 0.1 }).normalize();
        self.launch(&profile, None, Form::Bolt, from, along * TORRENT_SPEED * vpm, base * if surge { TORRENT_SURGE } else { TORRENT_BITE }, 0.0);
        self.signed(Sig::Power(Shape::Spark));
        if let Some(p) = self.projectiles.last_mut() {
            p.range = (to - from).norm() + vpm;
            p.seek = seek;
            p.size = if surge { 1.6 } else { 1.0 };
        }
        // A fork of it crackling out of the weapon after it
        if surge || random(&mut self.rng) < 0.5 {
            let reach = 1.2 + 1.2 * random(&mut self.rng);
            self.lightning(from, from + along * reach * vpm, storm, if surge { 0.04 } else { 0.025 }, 1, 1, 0.0, 0.0);
        }
        self.cues.push(Cue::here(Sound::Zap, if surge { 0.5 } else { 0.18 }).pitched(1.1 + 0.5 * random(&mut self.rng) as f32));
    }

    /// One of their shots on its way: a runner held to the ground and crackling, a Twin Arc's
    /// chakrams out and back, cutting what their line crosses, sparks streaming off the rest.
    /// Some(true) if it's gone, None for the rest of its flight to go on as any power's does.
    pub(in crate::combat) fn thunder_flies(&mut self, arena: &mut Arena, p: &mut Projectile, shape: Shape, dt: f64) -> Option<bool> {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let storm = Element::look(Some(Element::Storm));
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        match shape {
            // Racing along the ground, up slopes and down off ledges, crackling as it goes
            Shape::Runner => {
                let ground = ground_under(arena.world, p.position - up * RUNNER_HIGH * vpm, vpm);
                p.position.z = ground.z + RUNNER_HIGH * vpm;
                p.velocity.z = 0.0;
                if random(&mut self.rng) < dt * 16.0 {
                    let to = p.position - way * (0.8 + 0.8 * random(&mut self.rng)) * vpm + self.any_direction() * 0.3 * vpm;
                    self.lightning(p.position, to, storm, 0.03, 0, 1, 0.0, 0.0);
                }
                if random(&mut self.rng) < dt * 40.0 {
                    let v = (self.any_direction() + up) * 1.5 * vpm;
                    self.fine(p.position, v, storm, 0.25, 0.6);
                }
                // Running in under what it's after, low as it is: bursting up into it
                let caught = (0..arena.animals.len()).find(|&i| {
                    let a = &arena.animals[i];
                    let to = a.center(vpm as u32) - p.position;
                    a.health > 0.0 && !a.look.is_pet() && !p.hit.contains(&a.id) && to.xy().norm() < (0.5 + a.look.body().1) * vpm && to.z.abs() < 2.5 * vpm
                });
                if let Some(i) = caught {
                    let at = p.position;
                    return Some(self.thunder_strikes(arena, p, shape, i, at));
                }
                None
            }
            Shape::Twin { back, reach } => Some(self.twin_flies(arena, p, back, reach, dt)),
            Shape::Volt { .. } | Shape::Rail | Shape::Spark | Shape::Summon | Shape::Grave => {
                if random(&mut self.rng) < dt * 25.0 {
                    let at = p.position + self.any_direction() * 0.2 * vpm;
                    self.mote(at, Some(if shape == Shape::Grave { Element::Void } else { Element::Storm }), 1.2);
                }
                // The arrow crackling as it flies
                if shape == (Shape::Volt { split: true }) && random(&mut self.rng) < dt * 14.0 {
                    let to = p.position - way * 1.2 * vpm + self.any_direction() * 0.35 * vpm;
                    self.lightning(p.position, to, storm, 0.025, 0, 1, 0.0, 0.0);
                }
                None
            }
            _ => None,
        }
    }

    /// A Twin Arc's pair on its way, `back` or not, out `reach` meters: turning back once it's as
    /// far as it goes, caught again at the hand, and what the line between them crosses shocked,
    /// once each way. True if it's gone.
    fn twin_flies(&mut self, arena: &mut Arena, p: &mut Projectile, back: bool, reach: u8, dt: f64) -> bool {
        let vpm = arena.view.vpm;
        let storm = Element::look(Some(Element::Storm));
        let out = reach as f64 * vpm;
        // Never landed as far as it goes, as other shots are: turned back here instead
        p.range = f64::INFINITY;
        let mut back = back;
        if !back && p.flown >= out {
            back = true;
            p.trick = Trick::Signature(Sig::Power(Shape::Twin { back: true, reach }));
            p.hit.clear();
            self.landed(p.position);
            self.cues.push(Cue::at(Sound::Zap, p.position, 0.7).pitched(1.5));
        }
        if back {
            let hand = arena.view.eye + arena.view.rotation * Vector3::new(0.15, 0.5, -0.25) * vpm;
            let to = hand - p.position;
            if to.norm() < 1.4 * vpm {
                self.sparks(p.position, 10, 2.5, storm, 0.0, 0.3);
                self.cues.push(Cue::here(Sound::Clang, 0.5).pitched(1.5));
                return true;
            }
            p.velocity = to.normalize() * TWIN_SPEED * 1.25 * vpm;
        }
        let (a, b) = twin_ends(p, back, reach, vpm);
        let profile = p.profile.clone();
        let caught: Vec<usize> = (0..arena.animals.len())
            .filter(|&i| {
                let animal = &arena.animals[i];
                animal.health > 0.0 && !animal.look.is_pet() && !p.hit.contains(&animal.id) && off_segment(animal.center(vpm as u32), a, b) < (TWIN_CUT + animal.look.body().1) * vpm
            })
            .collect();
        for i in caught {
            let center = arena.animals[i].center(vpm as u32);
            p.hit.push(arena.animals[i].id);
            self.landed(center);
            let near = if (center - a).norm() < (center - b).norm() { a } else { b };
            self.lightning(near, center, storm, 0.05, 1, 2, 0.0, 0.0);
            let damage = p.damage * self.wet(arena, i);
            self.power_hit(arena, &profile, i, center, damage, p.position);
            if arena.animals[i].health > 0.0 {
                arena.animals[i].status.shocked = arena.animals[i].status.shocked.max(0.8);
            }
        }
        // The line between them crackling anew every few frames
        if random(&mut self.rng) < dt * 11.0 {
            self.lightning(a, b, storm, 0.04, 1, 1, 0.0, 0.0);
        }
        if random(&mut self.rng) < dt * 30.0 {
            let at = a.lerp(&b, random(&mut self.rng));
            let v = self.any_direction() * 1.2 * vpm;
            self.fine(at, v, storm, 0.2, 0.0);
        }
        false
    }

    /// A Thundersplit Arrow `p` splitting at `at` into four, each leaping on at the next nearest
    /// within SPLIT_LEAP it hasn't struck, or fanning on ahead with nothing there
    fn volt_splits(&mut self, arena: &Arena, p: &Projectile, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let storm = Element::look(Some(Element::Storm));
        let way = flat_of(p.velocity);
        let next: Vec<usize> = Combat::foes_round(arena, at, SPLIT_LEAP).into_iter().filter(|&j| !p.hit.contains(&arena.animals[j].id)).take(SPLITS).collect();
        for k in 0..SPLITS {
            let (dir, range, seek) = match next.get(k) {
                Some(&j) => {
                    let to = arena.animals[j].center(vpm as u32);
                    self.lightning(at, to, storm, 0.05, 1, 2, HOP_GAP * (k + 1) as f64, 0.0);
                    ((to - at).try_normalize(1e-6).unwrap_or(way), (to - at).norm() + 2.0 * vpm, 8.0)
                }
                None => {
                    let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), (k as f64 - 1.5) * 0.45);
                    ((turn * way + Vector3::new(0.0, 0.0, 0.05)).normalize(), 7.0 * vpm, 0.0)
                }
            };
            self.launch(&p.profile, None, Form::Bolt, at + dir * 0.5 * vpm, dir * SPLINTER_SPEED * vpm, p.damage * 0.35, 0.0);
            self.signed(Sig::Power(Shape::Volt { split: false }));
            if let Some(n) = self.projectiles.last_mut() {
                n.hit = p.hit.clone();
                n.range = range;
                n.seek = seek;
            }
        }
        self.flashes.push(Flash { position: at, color: Element::Storm.light(), reach: 7.0, life: 0.2, lifetime: 0.2 });
        self.cues.push(Cue::at(Sound::Thunder, at, 0.5).pitched(1.4));
        self.cues.push(Cue::at(Sound::Crackle, at, 0.8).pitched(1.0));
    }

    /// One of their shots striking animal `i` at `at`. True if it's gone.
    pub(in crate::combat) fn thunder_strikes(&mut self, arena: &mut Arena, p: &mut Projectile, shape: Shape, i: usize, at: Vector3<f64>) -> bool {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let storm = Element::look(Some(Element::Storm));
        let profile = p.profile.clone();
        let id = arena.animals[i].id;
        let from = p.position - p.velocity * 0.02;
        match shape {
            // Bursting up out of the ground into it
            Shape::Runner => {
                let center = arena.animals[i].center(vpm as u32);
                self.landed(center);
                let foot = Combat::floor_at(arena, arena.animals[i].position + up * 0.5 * vpm);
                self.lightning(foot, center + up * 0.6 * vpm, storm, 0.07, 2, 2, 0.0, 1.0);
                let damage = p.damage * self.wet(arena, i);
                self.power_hit(arena, &profile, i, center, damage, foot);
                if arena.animals[i].health > 0.0 {
                    arena.animals[i].status.shocked = arena.animals[i].status.shocked.max(0.8);
                }
                self.sparks(foot, 10, 4.0, storm, 0.6, 0.3);
                self.cues.push(Cue::at(Sound::Crackle, at, 0.7).pitched(1.2));
                true
            }
            // Struck, and splitting on into four
            Shape::Volt { split: true } => {
                p.hit.push(id);
                let damage = p.damage * self.wet(arena, i);
                self.power_hit(arena, &profile, i, at, damage, from);
                self.volt_splits(arena, p, at);
                true
            }
            Shape::Volt { split: false } => {
                let damage = p.damage * self.wet(arena, i);
                self.power_hit(arena, &profile, i, at, damage, from);
                self.flashes.push(Flash { position: at, color: Element::Storm.light(), reach: 4.0, life: 0.12, lifetime: 0.12 });
                true
            }
            // A torrent's spark: the lightning seen pouring from the hand into it
            Shape::Spark => {
                let hand = arena.view.eye + arena.view.rotation * Vector3::new(0.25, 0.6, -0.2) * vpm;
                self.lightning(hand, at, storm, if p.size > 1.2 { 0.05 } else { 0.035 }, 1, 1, 0.0, 0.0);
                let damage = p.damage * self.wet(arena, i);
                self.strike(arena, &profile, i, Blow { damage, crit: false, heavy: p.size > 1.2, from: hand, depth: 1, element: Some(Element::Storm) });
                if arena.animals[i].health > 0.0 {
                    let s = &mut arena.animals[i].status;
                    s.shocked = s.shocked.max(0.4);
                }
                self.sparks(at, 4, 3.0, storm, 0.4, 0.25);
                true
            }
            // Through everything in its line
            Shape::Rail => {
                p.hit.push(id);
                let damage = p.damage * self.wet(arena, i);
                self.power_hit(arena, &profile, i, at, damage, from);
                if arena.animals[i].health > 0.0 {
                    arena.animals[i].status.shocked = arena.animals[i].status.shocked.max(1.0);
                }
                self.flashes.push(Flash { position: at, color: Element::Storm.light(), reach: 5.0, life: 0.15, lifetime: 0.15 });
                false
            }
            // Its line does it, as it flies, see twin_flies(); what the pair's middle meets too
            Shape::Twin { .. } => {
                p.hit.push(id);
                let damage = p.damage * self.wet(arena, i);
                self.power_hit(arena, &profile, i, at, damage, from);
                false
            }
            // On up past it, to gather the storm
            Shape::Summon => false,
            Shape::Grave => {
                self.thunder_lands(arena, p, shape, at);
                true
            }
            _ => true,
        }
    }

    /// One of their shots coming down at `at`, or as far as it goes
    pub(in crate::combat) fn thunder_lands(&mut self, arena: &mut Arena, p: &Projectile, shape: Shape, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let storm = Element::look(Some(Element::Storm));
        let profile = p.profile.clone();
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        match shape {
            // Spent, a last fork leaping up out of the ground
            Shape::Runner => {
                self.power_pop(arena, &profile, at, 1.4, p.damage * 0.5, None);
                let to = at + (Vector3::new(0.0, 0.0, 1.6) + self.any_direction() * 0.4) * vpm;
                self.lightning(at, to, storm, 0.04, 1, 1, 0.0, 0.0);
                self.scorch(Combat::floor_at(arena, at), 0.5);
            }
            // Into the ground: lightning down out of the sky onto it, and splitting on from there
            Shape::Volt { split: true } => {
                let ground = Combat::floor_at(arena, at);
                self.sky_bolt(ground, storm, 0.8, 0.0);
                self.power_burst(arena, &profile, ground + up * 0.3 * vpm, 2.5, p.damage * 0.5, None);
                self.volt_splits(arena, p, ground + up * 0.6 * vpm);
            }
            Shape::Volt { split: false } => self.power_pop(arena, &profile, at, 1.0, p.damage * 0.5, None),
            Shape::Spark => {
                self.sparks(at, 3, 2.5, storm, 0.5, 0.2);
                if random(&mut self.rng) < 0.3 {
                    let to = at + (self.any_direction() + up) * 0.6 * vpm;
                    self.lightning(at, to, storm, 0.02, 0, 1, 0.0, 0.0);
                }
            }
            // The rail's end, bursting
            Shape::Rail => {
                self.power_burst(arena, &profile, at, 1.6, p.damage * 0.5, None);
                self.sparks(at, 16, 5.0, storm, 0.5, 0.35);
                self.scorch(Combat::floor_at(arena, at), 0.6);
            }
            // Glancing off a wall and coming back
            Shape::Twin { back: false, reach } => {
                self.sparks(at, 10, 3.0, storm, 0.4, 0.3);
                self.cues.push(Cue::at(Sound::Clang, at, 0.6).pitched(1.2));
                self.launch(&profile, None, Form::Bolt, at - way * 0.3 * vpm, -way * TWIN_SPEED * vpm, p.damage, 0.0);
                self.signed(Sig::Power(Shape::Twin { back: true, reach }));
                if let Some(n) = self.projectiles.last_mut() {
                    n.flown = reach as f64 * vpm;
                }
            }
            Shape::Twin { back: true, .. } => self.sparks(at, 10, 2.5, storm, 0.3, 0.3),
            // Gone up into the storm gathering over the ring
            Shape::Summon => {
                self.sparks(at, 14, 4.0, storm, 0.0, 0.4);
                self.flashes.push(Flash { position: at, color: Element::Storm.light(), reach: 9.0, life: 0.3, lifetime: 0.3 });
            }
            // The ground there breaking up in lightning
            Shape::Grave => {
                let ground = Combat::floor_at(arena, at);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Grave)), &profile, ground, Vector3::zeros(), GRAVE_ROUND, GRAVE_STRIKES as f64 * GRAVE_GAP + 0.4, p.damage);
                self.blasts.push(Blast { position: ground + up * 0.3 * vpm, look: Element::look(Some(Element::Void)), radius: 1.0, life: 0.3, lifetime: 0.3 });
                self.sparks(ground, 14, 3.0, Element::look(Some(Element::Void)), -0.2, 0.5);
                self.cues.push(Cue::at(Sound::Wail, ground, 0.45).pitched(0.7));
                self.cues.push(Cue::at(Sound::Thunder, ground, 0.5).pitched(1.2));
            }
            _ => {}
        }
    }

    /// One of their zones going on
    pub(in crate::combat) fn thunder_zone(&mut self, arena: &mut Arena, z: &mut Zone, kind: Lasting, dt: f64) {
        let vpm = arena.view.vpm;
        let storm = Element::look(Some(Element::Storm));
        match kind {
            // Short arcs crawling along the rail as it fades
            Lasting::Rail => {
                let fade = (z.life / z.lifetime).clamp(0.0, 1.0);
                let length = z.velocity.norm();
                if length > vpm && random(&mut self.rng) < dt * 20.0 * fade {
                    let way = z.velocity / length;
                    let from = z.position + z.velocity * random(&mut self.rng);
                    let to = from + way * (0.8 + random(&mut self.rng)) * vpm + self.any_direction() * 0.3 * vpm;
                    self.lightning(from, to, storm, 0.03, 0, 1, 0.0, 0.0);
                }
            }
            Lasting::Judgment => self.judgment(arena, z, dt),
            Lasting::Grave => self.grave_breaks(arena, z),
            _ => {}
        }
    }

    /// A Judgment Bolt's storm over the ring: lightning crawling in over the ground under it as it
    /// gathers, then the great bolt coming down, striking everything round, hardest in the middle,
    /// and throwing it back, with a flash lighting everything round, a pillar of light where it
    /// struck and the loudest thunder there is
    fn judgment(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let storm = Element::look(Some(Element::Storm));
        let age = z.lifetime - z.life;
        if z.owed > 0.0 {
            return;
        }
        if age < JUDGE_GATHER {
            let f = age / JUDGE_GATHER;
            if random(&mut self.rng) < dt * (10.0 + 22.0 * f) {
                let a = random(&mut self.rng) * TAU;
                let out = Vector3::new(a.cos(), a.sin(), 0.0);
                let r = z.radius * (0.6 + 0.4 * random(&mut self.rng));
                let from = z.position + out * r * vpm + up * 0.1 * vpm;
                let to = z.position + out * r * (0.25 + 0.3 * random(&mut self.rng)) * vpm + up * 0.1 * vpm;
                self.lightning(from, to, storm, 0.03, 1, 1, 0.0, 0.0);
            }
            if random(&mut self.rng) < dt * 30.0 {
                let a = random(&mut self.rng) * TAU;
                let at = z.position + Vector3::new(a.cos(), a.sin(), 0.05) * z.radius * random(&mut self.rng) * vpm;
                self.fine(at, up * (2.0 + 2.0 * f) * vpm, storm, 0.4, -0.2);
            }
            return;
        }
        z.owed = 1.0;
        let ground = z.position;
        let top = ground + up * (JUDGE_HEIGHT + 4.0) * vpm;
        self.lightning(top, ground, storm, 0.34, 6, 3, 0.0, 3.0);
        for k in 0..4 {
            let a = k as f64 / 4.0 * TAU + random(&mut self.rng);
            let to = ground_under(arena.world, ground + Vector3::new(a.cos(), a.sin(), 0.0) * (2.0 + 1.5 * random(&mut self.rng)) * vpm + up * vpm, vpm);
            self.lightning(top + Vector3::new(a.cos(), a.sin(), 0.0) * vpm, to, storm, 0.09, 2, 2, 0.03 * (k + 1) as f64, 0.0);
        }
        let profile = z.profile.clone();
        for i in Combat::foes_round(arena, ground, z.radius) {
            let center = arena.animals[i].center(vpm as u32);
            let d = (center - ground).xy().norm() / vpm;
            let share = if d <= JUDGE_CORE { 1.0 } else { 1.0 - 0.5 * ((d - JUDGE_CORE) / (z.radius - JUDGE_CORE)).min(1.0) };
            let damage = z.damage * share * self.wet(arena, i);
            self.landed(center);
            self.strike(arena, &profile, i, Blow { damage, crit: false, heavy: true, from: ground, depth: 0, element: Some(Element::Storm) });
            let a = &mut arena.animals[i];
            a.status.shocked = a.status.shocked.max(1.5);
            if a.health > 0.0 {
                a.status.staggered = a.status.staggered.max(if a.boss.is_some() { 0.3 } else { 0.8 });
                if a.boss.is_none() {
                    let away = (a.position - ground).xy().try_normalize(1e-6).map_or(Vector3::zeros(), |v| Vector3::new(v.x, v.y, 0.5));
                    a.status.push = away * 7.0 * vpm;
                } else {
                    a.status.limit_for_boss();
                }
            }
        }
        self.blasts.push(Blast { position: ground + up * 0.4 * vpm, look: storm, radius: 1.8, life: 0.35, lifetime: 0.35 });
        if (ground - arena.view.eye).norm() > 4.0 * vpm {
            self.swells.push(Swell { model: PILLAR, position: ground, radius: 1.3, life: 0.55, lifetime: 0.55 });
        }
        self.flashes.push(Flash { position: ground + up * 2.0 * vpm, color: Element::Storm.light(), reach: 20.0, life: 0.5, lifetime: 0.5 });
        self.sparks(ground + up * 0.2 * vpm, 30, 7.0, storm, 0.8, 0.5);
        for _ in 0..12 {
            let v = (self.any_direction() * 1.5 + up) * 5.0 * vpm;
            self.fine(ground + up * 0.1 * vpm, v, storm, 0.5, 0.6);
        }
        self.scorch(ground, 2.2);
        self.cues.push(Cue::at(Sound::Thunder, ground, 1.0).pitched(0.6));
        self.cues.push(Cue::at(Sound::Boom, ground, 0.8).pitched(0.6));
        let near = (1.0 - (ground - arena.view.eye).norm() / vpm / 40.0).max(0.0);
        self.shake = self.shake.max(0.3 + 0.6 * near);
    }

    /// Grave Lightning breaking up out of the ground one strike after another, under each enemy in
    /// the ring first, then round about it, what it strikes giving back some of its life along a
    /// thread of blood
    fn grave_breaks(&mut self, arena: &mut Arena, z: &mut Zone) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let storm = Element::look(Some(Element::Storm));
        let void = Element::look(Some(Element::Void));
        let blood = Element::look(Some(Element::Blood));
        let age = z.lifetime - z.life;
        let profile = z.profile.clone();
        while (z.owed as usize) < GRAVE_STRIKES && age >= z.owed * GRAVE_GAP {
            let k = z.owed as usize;
            z.owed += 1.0;
            let next = Combat::foes_round(arena, z.position, z.radius).into_iter().find(|&i| !z.hit.contains(&arena.animals[i].id));
            let spot = match next {
                Some(i) => {
                    z.hit.push(arena.animals[i].id);
                    Combat::floor_at(arena, arena.animals[i].position + up * 0.5 * vpm)
                }
                None => {
                    let a = k as f64 * 2.4;
                    let r = z.radius * (0.3 + 0.6 * ((k * 7) % 10) as f64 / 10.0);
                    Combat::floor_at(arena, z.position + Vector3::new(a.cos(), a.sin(), 0.0) * r * vpm + up * vpm)
                }
            };
            let lean = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, 0.0) * 2.0 * vpm;
            let top = spot + up * (5.0 + 1.5 * random(&mut self.rng)) * vpm + lean;
            self.lightning(spot, top, storm, 0.09, 3, 2, 0.0, if k % 3 == 0 { 1.2 } else { 0.0 });
            let off = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, 0.0) * 0.5 * vpm;
            self.lightning(spot + off, top * 0.85 + spot * 0.15 + off, void, 0.05, 1, 1, 0.03, 0.0);
            self.sparks(spot, 8, 3.5, storm, -0.3, 0.35);
            self.mote(spot + up * 0.3 * vpm, Some(Element::Void), 1.6);
            self.cues.push(Cue::at(Sound::Crackle, spot, 0.6).pitched(0.9));
            if k % 2 == 0 {
                self.cues.push(Cue::at(Sound::Thunder, spot, 0.35).pitched(1.5));
            }
            let chest = arena.view.eye + arena.view.rotation * Vector3::new(0.0, 1.1, -0.6) * vpm;
            for i in Combat::foes_round(arena, spot, GRAVE_HIT) {
                let center = arena.animals[i].center(vpm as u32);
                let damage = z.damage * self.wet(arena, i);
                self.landed(center);
                self.strike(arena, &profile, i, Blow { damage, crit: false, heavy: false, from: spot, depth: 1, element: Some(Element::Storm) });
                if arena.animals[i].health > 0.0 {
                    arena.animals[i].status.shocked = arena.animals[i].status.shocked.max(0.8);
                }
                self.heal += damage * GRAVE_DRAIN;
                self.lightning(center, chest, blood, 0.025, 0, 1, 0.1, 0.0);
            }
        }
    }

    /// How their shots are drawn: runners as balls of lightning with their crackling trail along
    /// the ground, the arrow and its splinters, the rail's slug, the twin chakrams with their line
    /// between them, motes, sparks
    pub(in crate::combat) fn thunder_shot_instances(&self, p: &Projectile, shape: Shape, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let storm = Element::look(Some(Element::Storm));
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
        let far = ((p.position - eye).norm() / vpm / 15.0).max(1.0).sqrt();
        let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 11.0);
        let ball = |at: Vector3<f64>, look: usize, size: f64| {
            let (model, ball) = self.effect_model(SLOT_FIREBALL, look);
            (eye, place(model, ball, MODEL_RES, Vector3::zeros(), at, spin, vpm, size * far, MOVING))
        };
        let mut out = Vec::new();
        match shape {
            Shape::Runner => {
                let flicker = 1.0 + 0.2 * (self.time * 41.0 + p.position.x).sin();
                out.push(ball(p.position, storm, 0.18 * flicker));
                let back = (p.flown / vpm).min(2.6) * vpm;
                out.extend(self.line_bolts(eye, p.position - way * back, p.position, storm, 0.45, 1.4 * far, 0.14));
            }
            Shape::Volt { split: true } => {
                // Not right at the eye as it leaves the bow, where it'd fill the view
                if (p.position - eye).norm() > 1.5 * vpm {
                    let (model, arrow) = self.effect_model(SLOT_ARROW + p.head.min(ARROW_HEADS - 1), storm);
                    out.push((eye, place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), p.position, rotation, vpm, 1.5 * far, MOVING)));
                }
                let back = (p.flown / vpm).min(2.6) * vpm;
                out.extend(self.line_bolts(eye, p.position - way * back, p.position - way * 0.3 * vpm, storm, 0.4, 1.2 * far, 0.12));
                out.push(ball(p.position, storm, 0.06));
            }
            Shape::Volt { split: false } | Shape::Spark => {
                let big = if shape == Shape::Spark { 0.055 * p.size } else { 0.08 };
                out.push(ball(p.position, storm, big));
                let back = (p.flown / vpm).min(1.3) * vpm;
                out.extend(self.line_bolts(eye, p.position - way * back, p.position, storm, 0.45, 0.9 * far, 0.1));
            }
            Shape::Rail => {
                out.push(ball(p.position, storm, 0.1));
                out.extend(self.streak_at(eye, p.position, way, storm, 1.6 * far));
            }
            // The two chakrams, spinning, the lightning strung between them
            Shape::Twin { back, reach } => {
                let (a, b) = twin_ends(p, back, reach, vpm);
                for end in [a, b] {
                    // Not right at the eye as they leave the hand or come back to it
                    if (end - eye).norm() < 1.3 * vpm {
                        continue;
                    }
                    match self.flung() {
                        Some(f) => out.push((eye + (end - eye) * 0.1, f.instance(end, crate::combat::thrown::chakram_turn(way, self.time), vpm, far.min(1.6), MOVING))),
                        None => out.push(ball(end, storm, 0.1)),
                    }
                    out.extend(self.circle_instances(eye, end, 0.3, storm, 0.5 * far, self.time * 20.0));
                }
                out.extend(self.line_bolts(eye, a, b, storm, 0.45, 1.1 * far, 0.15));
            }
            Shape::Summon => {
                out.push(ball(p.position, storm, 0.16));
                out.extend(self.streak_at(eye, p.position - way * 0.3 * vpm, way, storm, 1.4 * far));
            }
            Shape::Grave => {
                let void = Element::look(Some(Element::Void));
                out.push(ball(p.position, void, 0.15));
                out.push(ball(p.position, storm, 0.07));
                out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.35 * far, storm, 0.6 * far, self.time * 9.0));
            }
            _ => {}
        }
        out
    }

    /// How their zones are drawn: the rail, dead straight, with coils of lightning round it fading
    /// along it; the storm gathering over the ring and drawing in on it, then the ring the great
    /// bolt throws out; the grave's circle
    pub(in crate::combat) fn thunder_zone_instances(&self, z: &Zone, kind: Lasting, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let up = Vector3::z();
        let storm = Element::look(Some(Element::Storm));
        let age = z.lifetime - z.life;
        let fade = (z.life / z.lifetime).clamp(0.0, 1.0);
        let mut out = Vec::new();
        match kind {
            Lasting::Rail => {
                let length = z.velocity.norm();
                if length < vpm {
                    return out;
                }
                let way = z.velocity / length;
                // The rail in a few straight stretches, each as thick as it is far off, thinning
                let pieces = 5;
                for k in 0..pieces {
                    let a = z.position + z.velocity * (k as f64 / pieces as f64);
                    let b = z.position + z.velocity * ((k + 1) as f64 / pieces as f64);
                    let d = ((a + b) * 0.5 - eye).norm() / vpm;
                    let thick = 0.15 * (0.3 + 0.7 * fade) * (d / 20.0).max(1.0).sqrt() * (d / 2.0).clamp(0.35, 1.0);
                    out.extend(self.bar(a, b, thick, storm).map(|i| ((a + b) * 0.5, i)));
                }
                // Coils round it, widening as they fade, nearest first, a few
                let coils = ((length / vpm / 4.0) as usize).clamp(1, 4);
                for k in 0..coils {
                    let at = z.position + way * (2.0 + k as f64 * (length / vpm - 2.0) / coils as f64).min(length / vpm) * vpm;
                    out.extend(self.ring_instances(eye, at, way, 0.5 + 0.8 * (1.0 - fade), storm, 1.0 * fade, age * 9.0 + k as f64));
                }
            }
            Lasting::Judgment => {
                if z.owed <= 0.0 {
                    let f = (age / JUDGE_GATHER).clamp(0.0, 1.0);
                    // The ring on the ground drawing in, and the storm turning over it, tightening
                    out.extend(self.circle_instances(eye, z.position + up * 0.08 * vpm, z.radius * (1.0 - 0.45 * f), storm, 1.1 + 0.6 * f, age * 1.5));
                    let cloud = z.position + up * (JUDGE_HEIGHT - 0.5) * vpm;
                    out.extend(self.circle_instances(eye, cloud, 3.2 - 1.6 * f, storm, 1.4, -age * 3.0));
                    // The leader reaching down out of it at the last
                    if f > 0.7 {
                        out.extend(self.line_bolts(eye, cloud, z.position + up * (JUDGE_HEIGHT * (1.0 - (f - 0.7) / 0.3)).max(0.2) * vpm, storm, 0.8, 0.9, 0.3));
                    }
                } else {
                    // The ring the bolt throws out, rolling out fast and fading
                    let since = age - JUDGE_GATHER;
                    let reached = (0.6 + 22.0 * since).min(z.radius);
                    out.extend(self.circle_instances(eye, z.position + up * 0.15 * vpm, reached, storm, 2.2 * (1.0 - since / 0.5).max(0.1), since * 3.0));
                }
            }
            Lasting::Grave => {
                let void = Element::look(Some(Element::Void));
                let fade = (z.life / 0.4).clamp(0.1, 1.0);
                out.extend(self.circle_instances(eye, z.position + up * 0.06 * vpm, z.radius, void, 1.3 * fade, age * 0.9));
                out.extend(self.circle_instances(eye, z.position + up * 0.06 * vpm, z.radius * 0.5, storm, 0.9 * fade, -age * 1.6));
            }
            _ => {}
        }
        out
    }

    /// What their zones light round them
    pub(in crate::combat) fn thunder_light(&self, z: &Zone, kind: Lasting) -> Option<(Vector3<f64>, f64, [f32; 3], f32)> {
        let flicker = 0.7 + 0.3 * (self.time * 29.0).sin().abs() as f32;
        let fade = (z.life / z.lifetime).clamp(0.0, 1.0) as f32;
        let up = Vector3::new(0.0, 0.0, self.vpm);
        let storm = Element::Storm.light();
        match kind {
            Lasting::Rail => Some((z.position + z.velocity * 0.3, 8.0, storm, 1.0 * fade)),
            Lasting::Judgment if z.owed <= 0.0 => {
                let f = ((z.lifetime - z.life) / JUDGE_GATHER).clamp(0.0, 1.0) as f32;
                Some((z.position + up, 7.0, storm, (0.3 + 0.6 * f) * flicker))
            }
            Lasting::Judgment => Some((z.position + up * 2.0, 18.0, storm, 2.2 * (z.life / 0.5).clamp(0.0, 1.0) as f32)),
            Lasting::Grave => Some((z.position + up, 6.0, storm, 0.7 * flicker * (z.life / 0.4).clamp(0.0, 1.0) as f32)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapons::Library;

    /// Open ground, 48 m across, four meters of earth under four of sky
    fn field() -> World {
        let size = [384u32, 384, 64];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 4].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, crate::worldgen::material::DIRT);
        world.voxels_per_meter = 8;
        world
    }

    const VPM: f64 = 8.0;

    fn feet() -> Vector3<f64> {
        Vector3::new(24.0, 10.0, 4.0) * VPM
    }

    fn view(pitch: f64) -> View {
        View { eye: feet() + Vector3::new(0.0, 0.0, 1.6 * VPM), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), pitch), tan: [1.2445, 0.7002], vpm: VPM, speed: 0.0, stride: 0.0 }
    }

    /// What using `part`'s power as a player would does to a pack of four `ahead` meters off, the
    /// crosshair on it, right click held `hold` seconds: how many of the pack it hurt, the combat
    /// after, and how much health it gave back
    fn used(part: &str, ahead: f64, hold: f64, pitch: f64) -> (usize, Combat, f64) {
        let library = Library::load();
        let world = field();
        let weapon = library.with_part(part, None).unwrap_or_else(|| panic!("no weapon with {part}"));
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let view = view(pitch);
        let mut animals = crate::animals::spawn_pack(&world, &dry, &[Look::Bandit; 4], feet() + Vector3::new(0.0, ahead * VPM, 0.0), crate::animals::PACK_SPREAD, 5);
        let mut combat = Combat::new(1, 0);
        combat.mana = 1000.0;
        let dt = 1.0 / 120.0;
        let mut healed = 0.0;
        for frame in 0..(3.8 / dt) as usize {
            let t = frame as f64 * dt - 0.4;
            let mut controls = combat.power_controls(&weapon, t, dt);
            if t >= 0.0 && t < hold {
                controls.aiming = true;
            }
            let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet()), water: &dry };
            combat.update(&mut arena, Some((0, &weapon)), None, controls, dt);
            healed += combat.heal;
            combat.heal = 0.0;
        }
        (animals.iter().filter(|a| a.health < a.max_health).count(), combat, healed)
    }

    /// Each strikes as it should at a pack: a blade's arcs and a slam's runners going after several
    /// of it, the arrow splitting on to the rest, the rail through what's in its line, the twin
    /// chakrams' line through it, the judgment bolt on all of it round the ring, the torrent at
    /// what's ahead, and the grave's lightning on what's in the ring, giving life back
    #[test]
    fn the_second_storm_strikes_the_pack() {
        for (part, least, hold, pitch) in [
            ("stormbrand_core", 3, 0.0, -0.12),
            ("stormrunner_core", 3, 0.0, -0.12),
            ("thundersplit_string", 3, 0.0, -0.07),
            ("railshot_lock", 1, 0.0, -0.07),
            ("railshot_sight", 1, 0.0, -0.07),
            ("twinarc_chakram", 2, 0.0, -0.07),
            ("judgment_crystal", 3, 0.0, -0.15),
            ("torrent_focus", 2, 1.2, -0.07),
            ("gravecall_binding", 3, 0.0, -0.15),
            ("gravecall_scythe", 3, 0.0, -0.15),
        ] {
            let (hurt, _, healed) = used(part, 8.5, hold, pitch);
            assert!(hurt >= least, "{part}'s power hurt only {hurt} of the pack");
            if part.starts_with("gravecall") {
                assert!(healed > 0.0, "{part}'s lightning gave no life back");
            }
        }
    }

    /// A blade the Stormbrand charged stays charged a while, and its blows call lightning down,
    /// hurting what's struck more than the same blow uncharged would
    #[test]
    fn a_charged_blade_calls_lightning_down() {
        let library = Library::load();
        let world = field();
        let mut weapon = library.with_part("stormbrand_core", None).unwrap();
        assert_eq!(weapon.profile.power, Some(Power::Stormbrand));
        // No critical hits, so the blows are alike but for the lightning
        Rc::make_mut(&mut weapon.profile).stats.insert("critChance".into(), 0.0);
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let view = view(-0.2);
        // One blow at a bandit right in front, charged or not
        let blow = |charged: bool| {
            let mut animals = crate::animals::spawn_pack(&world, &dry, &[Look::Bandit; 1], feet() + Vector3::new(0.0, 1.6 * VPM, 0.0), 0.0, 5);
            let start = animals[0].health;
            let mut combat = Combat::new(1, 0);
            combat.charged_blade = if charged { BRAND_SECONDS } else { 0.0 };
            let dt = 1.0 / 120.0;
            let bolts = combat.bolts.len();
            let mut most_bolts = bolts;
            for frame in 0..150 {
                let controls = Controls { attack: (60..62).contains(&frame), ..Default::default() };
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet()), water: &dry };
                combat.update(&mut arena, Some((0, &weapon)), None, controls, dt);
                most_bolts = most_bolts.max(combat.bolts.len());
            }
            (start - animals[0].health, most_bolts, combat.charged_blade)
        };
        let (plain, _, _) = blow(false);
        let (charged, bolts, left) = blow(true);
        assert!(plain > 0.0, "the blow never landed");
        assert!(charged > plain * 1.2, "charged it did {charged:.1}, uncharged {plain:.1}");
        assert!(bolts >= 1, "no lightning came down");
        assert!(left > BRAND_SECONDS - 1.5 && left < BRAND_SECONDS - 1.0, "the charge ran down to {left:.2} s in 1.25 s");
    }

    /// A torrent pours while right click's held, draining mana as it does, and lets go of itself
    /// once it's poured as long as it can
    #[test]
    fn a_torrent_pours_while_held_and_stops_itself() {
        let (_, combat, _) = used("torrent_focus", 8.5, 5.0, -0.07);
        assert!(combat.mana < 1000.0 - 1.0, "pouring it drained no mana");
        assert!(combat.power.cooldown > 0.0, "held past {TORRENT_MOST} s it never let go of itself");
    }

    /// The new parts are where they should be: every power of the second storm on some part, each
    /// for the families of the classes it's meant for, legendary or rarer, and none of them a spear
    #[test]
    fn the_second_storm_is_spread_over_the_classes() {
        let library = Library::load();
        for power in Power::ALL.into_iter().filter(|p| p.thunder()) {
            assert!(!power.spear(), "{power:?} is a spear, which are a priest's alone");
            assert!(power.lightning(), "{power:?} isn't lightning");
            let parts: Vec<_> = library.components.iter().filter(|c| c.power() == Some(power)).collect();
            assert!(!parts.is_empty(), "{power:?} is on no part");
            for c in parts {
                assert!(c.tier >= 4 && c.batch() > 0, "{} isn't a later legendary part", c.id);
            }
        }
        let on = |family: &str| library.components.iter().filter(|c| c.fits(family) && c.power().is_some_and(Power::thunder)).count();
        for class in [crate::character::Class::Archer, crate::character::Class::Fighter, crate::character::Class::Priest, crate::character::Class::Mage, crate::character::Class::Rogue, crate::character::Class::Necromancer] {
            assert!(class.kit().iter().any(|f| on(f) > 0), "{class:?} starts with nothing that can have the new lightning");
        }
    }
}
