//! How the dead raised by a skull focus or a necromancer's summonings are drawn, see
//! signatures\faith.rs and signatures\summons.rs. What they do is worked out there a frame at a time;
//! what's drawn of it is eased after it here: turning round smoothly rather than snapping to face
//! what they're after, easing into and out of their stride, their feet following the ground rather
//! than stepping up and down a voxel at a time, winding up, striking and getting over each blow
//! through the poses between, leaning up out of the ground as they climb out of it and slumping as
//! they crumble back into it. Near the camera they're posed live from their pieces, as the monsters
//! they were are, see baked::Rig::live(); further off they're drawn in the baked frame nearest.

use super::signatures::faith::{CRUMBLE_SECONDS, RISE_SECONDS, Risen};
use super::*;
use crate::animals::{self, POSE_STRIKE, POSE_WINDUP};
use crate::baked::Doing;
use crate::rig::smooth;

/// How quickly what's drawn turns after where it faces, eases into and out of its stride, and its
/// feet follow the ground: about the time each takes, inverted
const TURN_EASE: f64 = 12.0;
const STRIDE_EASE: f64 = 9.0;
const GROUND_EASE: f64 = 20.0;
/// How far through a blow, or drawing a bow, it's wound up, lands or looses, see summons.rs, and has
/// begun to get over it
const WOUND: f64 = 0.4;
const LANDS: f64 = 0.55;
const OVER: f64 = 0.66;
/// Seconds a blow cut short takes to fade back out of
const CUT_SHORT: f64 = 0.2;
/// How far forward it leans climbing up out of the ground, and slumps crumbling back into it, in
/// radians
const CLIMB_LEAN: f64 = 0.35;
const SLUMP: f64 = 0.5;

/// What's drawn of one of the dead raised, eased from frame to frame after what it's doing
#[derive(Clone, Debug)]
pub(super) struct Drawn {
    look: Look,
    /// When it was raised, by the fight's clock, so it's known again from one frame to the next
    born: f64,
    /// Which way it's drawn facing, in radians; how much it's striding, 0 standing to 1; and how
    /// high its feet are drawn, in voxels
    heading: f64,
    stride: f64,
    z: f64,
    /// How far into its blow it was last frame, if it was swinging; and where a blow cut short got
    /// to, and seconds since
    blow: Option<f64>,
    cut: Option<(f64, f64)>,
    /// How much it's posed live from its pieces rather than drawn in its baked frame, 0 to 1, eased
    /// in and out as it comes near enough and goes too far, see Game::choose_live()
    live: f64,
}

/// One of the dead raised as it's to be drawn this frame, see Combat::raised_bodies()
pub struct RaisedBody {
    pub look: Look,
    /// Where its feet are drawn and how it's turned, in voxels of the world, and how many voxels of
    /// the world a meter of it is
    pub feet: Vector3<f64>,
    pub turn: UnitQuaternion<f64>,
    pub meter: f64,
    /// How far it is from the camera, in voxels
    pub distance: f64,
    /// Its baked frame nearest to how it's posed, standing in for it in shadows and reflections
    /// once it's posed live
    pub frame: AnimalInstance,
    /// Which frame that is, and what it's doing, to pose it live
    pub frame_at: usize,
    pub doing: Doing,
    /// How much it's posed live, 0 to 1, see Drawn::live, and which of those eased from frame to
    /// frame it is, if it's one yet, see Combat::pose_raised_live()
    pub live: f64,
    pub slot: Option<usize>,
    pub tint: u32,
}

impl RaisedBody {
    /// Its pieces posed live as it is now, into `out`, see baked::Rig::live(): false if it can't be
    pub fn live_instances(&self, out: &mut Vec<AnimalInstance>) -> bool {
        if !crate::baked::posable(self.look) {
            return false;
        }
        let Some(rig) = crate::baked::rig(self.look) else { return false };
        let poses = rig.live_from(&self.doing, self.frame_at, self.live);
        if poses.is_empty() {
            return false;
        }
        let start = out.len();
        rig.draw_live(&poses, self.feet, self.turn, self.meter, out);
        for piece in &mut out[start..] {
            *piece = piece.tinted(self.tint);
        }
        out.len() > start
    }
}

/// The poses `look` goes through `s` of the way through a blow, or drawing a bow, see Doing::pose:
/// winding up from standing or its stride, the blow coming down quicker and quicker to land hard,
/// held a moment, then back
fn blow_pose(look: Look, s: f64) -> (Vec<usize>, f64) {
    let path = |a: usize, b: usize| animals::tween_path(look, a, b, 0.0).map_or_else(|| vec![a, b], |(p, _)| p);
    if s < WOUND {
        (path(0, POSE_WINDUP), smooth(s / WOUND))
    } else if s < LANDS {
        let f = (s - WOUND) / (LANDS - WOUND);
        (path(POSE_WINDUP, POSE_STRIKE), f * f)
    } else if s < OVER {
        (vec![POSE_STRIKE], 0.0)
    } else {
        (path(POSE_STRIKE, 0), smooth((s - OVER) / (1.0 - OVER)))
    }
}

/// A blow cut short `s` of the way through, `t` seconds ago, fading back out of where it got to
fn cut_pose(look: Look, s: f64, t: f64) -> (Vec<usize>, f64) {
    let back = smooth(t / CUT_SHORT);
    let (path, along) = blow_pose(look, s);
    if s < WOUND {
        // Unwinding what it had wound up
        (path, along * (1.0 - back))
    } else if s < LANDS {
        // The blow coming down carried on through and back, from where it got to
        let (after, _) = blow_pose(look, 1.0 - 1e-6);
        let m = path.len() - 1;
        let whole: Vec<usize> = path.iter().chain(after.iter().skip(1)).copied().collect();
        let from = along * m as f64 / (whole.len() - 1) as f64;
        (whole, from + (1.0 - from) * back)
    } else {
        let (after, _) = blow_pose(look, 1.0 - 1e-6);
        (after, back)
    }
}

/// The baked frame nearest to how something doing `doing` is posed live: the pose along its path
/// it's nearest, or of its walk if it's striding, or standing
fn nearest_frame(doing: &Doing) -> usize {
    if let Some((path, along)) = &doing.pose
        && !path.is_empty()
    {
        let n = path.len() - 1;
        let f = path[((along.clamp(0.0, 1.0) * n as f64).round() as usize).min(n)];
        if f != 0 {
            return f;
        }
    }
    if doing.stride > 0.5 { animals::walk_frame(doing.phase) } else { 0 }
}

impl Drawn {
    fn new(r: &Risen, f: &Familiar, born: f64) -> Drawn {
        Drawn { look: r.look, born, heading: r.heading, stride: 0.0, z: f.position.z, blow: None, cut: None, live: 0.0 }
    }

    /// Eased `dt` seconds on after what `f`, raised as `r`, is doing now
    fn ease(&mut self, f: &Familiar, r: &Risen, vpm: f64, dt: f64) {
        let off = (r.heading - self.heading + PI).rem_euclid(TAU) - PI;
        let rate = TURN_EASE * (1.0 + 2.0 * off.abs());
        self.heading = (self.heading + off * (1.0 - (-rate * dt).exp())).rem_euclid(TAU);
        let striding = if r.moving { 1.0 } else { 0.0 };
        self.stride += (striding - self.stride) * (1.0 - (-STRIDE_EASE * dt).exp());
        // Its feet after the ground, quickly but not a voxel at a time; and straight there if it's
        // been put somewhere else altogether
        let rise = f.position.z - self.z;
        self.z = if rise.abs() > 2.0 * vpm { f.position.z } else { self.z + rise * (1.0 - (-GROUND_EASE * dt).exp()) };
        // A blow cut short, as its time's out and it crumbles, fading out from where it got to
        match (r.swing, self.blow) {
            (Some(_), _) => self.cut = None,
            (None, Some(s)) if s < OVER => self.cut = Some((s, 0.0)),
            _ => {}
        }
        if let Some((_, t)) = &mut self.cut {
            *t += dt;
            if *t >= CUT_SHORT {
                self.cut = None;
            }
        }
        self.blow = r.swing;
    }

    /// What `r` is doing, as it's drawn, to pose it live
    fn doing(&self, r: &Risen) -> Doing {
        let pose = match (r.swing, self.cut) {
            (Some(s), _) => Some(blow_pose(r.look, s)),
            (None, Some((s, t))) => Some(cut_pose(r.look, s, t)),
            _ => None,
        };
        Doing { phase: r.phase, stride: self.stride, pose, age: r.age.max(0.0), seed: (self.born * 1000.0) as u32, ..Default::default() }
    }
}

impl Combat {
    /// What's drawn of each of the dead raised eased on `dt` seconds after what they're doing now:
    /// each known again from the last frame by what it was raised as and when, new ones as they
    /// are, and those gone forgotten
    pub(super) fn ease_raised(&mut self, dt: f64) {
        let vpm = self.vpm();
        let mut last = std::mem::take(&mut self.raised);
        for f in &self.familiars {
            let Some(r) = &f.risen else { continue };
            let born = self.time - r.age;
            let mut drawn = match last.iter().position(|d| d.look == r.look && (d.born - born).abs() < 1e-3) {
                Some(k) => last.swap_remove(k),
                None => Drawn::new(r, f, born),
            };
            drawn.ease(f, r, vpm, dt);
            self.raised.push(drawn);
        }
    }

    /// The world moved over by `shift` voxels
    pub(super) fn recenter_raised(&mut self, shift: Vector3<f64>) {
        for drawn in &mut self.raised {
            drawn.z -= shift.z;
        }
    }

    /// How `f`, one of the dead raised as `r`, is drawn now, seen from `view`: None while it's
    /// under the ground
    pub(in crate::combat) fn raised_body(&self, f: &Familiar, r: &Risen, view: &View) -> Option<RaisedBody> {
        let vpm = view.vpm;
        let born = self.time - r.age;
        let slot = self.raised.iter().position(|d| d.look == r.look && (d.born - born).abs() < 1e-3);
        let fresh;
        let drawn = match slot {
            Some(k) => &self.raised[k],
            None => {
                fresh = Drawn::new(r, f, born);
                &fresh
            }
        };
        // A grave brute's as tall again as a skeleton
        let height = if r.look == Look::GraveBrute { 3.3 } else { 1.9 } * vpm;
        let rising = (r.age / RISE_SECONDS).clamp(0.0, 1.0);
        let sinking = (f.life / CRUMBLE_SECONDS).clamp(0.0, 1.0);
        let shown = rising.min(sinking);
        if shown <= 0.0 {
            return None;
        }
        let ease = 1.0 - (1.0 - shown) * (1.0 - shown);
        let feet = Vector3::new(f.position.x, f.position.y, drawn.z - (1.0 - ease) * height);
        // Leaning forward as it climbs up out of the ground, straightening as it gets out, and
        // slumping forward as it crumbles back into it
        let lean = CLIMB_LEAN * (1.0 - smooth(rising)) + SLUMP * (1.0 - smooth(sinking));
        let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), drawn.heading) * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), lean);
        let doing = drawn.doing(r);
        let frame_at = nearest_frame(&doing);
        let tint = if f.life < 2.0 && (f.life * 8.0).fract() < 0.5 { TINT_HIT } else { TINT_NONE };
        let frame = animals::frame_instance(r.look, feet, turn, frame_at, vpm as u32).tinted(tint);
        Some(RaisedBody { look: r.look, feet, turn, meter: vpm, distance: (f.position - view.eye).norm(), frame, frame_at, doing, live: drawn.live, slot, tint })
    }

    /// Every one of the dead raised as it's to be drawn now, seen from `view`, see raised_body()
    pub fn raised_bodies(&self, view: &View) -> Vec<RaisedBody> {
        self.familiars.iter().filter_map(|f| self.raised_body(f, f.risen.as_ref()?, view)).collect()
    }

    /// Those of the dead raised in `wanted`, by RaisedBody::slot, posed live from their pieces and
    /// the rest not: each easing into or out of it over `dt` seconds, see Game::choose_live()
    pub fn pose_raised_live(&mut self, wanted: &[usize], dt: f64) {
        for (k, drawn) in self.raised.iter_mut().enumerate() {
            drawn.live = crate::baked::fade_live(drawn.live, wanted.contains(&k), dt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Through a whole blow, winding up, striking and getting over it, and through one cut short,
    /// the poses it goes through follow on from one moment to the next, ending where they began:
    /// taken a few thousandths of the way at a time, nothing moves as far as it
    /// would if it jumped, however quick the blow
    #[test]
    fn a_blow_goes_on_without_a_jump() {
        for look in [Look::Skeleton, Look::ArmoredSkeleton, Look::BoneArcher, Look::GraveBrute, Look::Zombie, Look::Bandit] {
            let rig = crate::baked::rig(look).unwrap();
            let pose = |pose: Option<(Vec<usize>, f64)>| rig.live(&Doing { pose, ..Default::default() });
            let standing = pose(None);
            let mut last = standing.clone();
            let steps = 2400;
            for s in 1..steps {
                let now = pose(Some(blow_pose(look, s as f64 / steps as f64)));
                let jump = crate::baked::furthest(&last, &now);
                assert!(jump < 0.08, "{look:?} jumps {jump:.3} m {s} of {steps} of the way through its blow");
                last = now;
            }
            assert!(crate::baked::furthest(&last, &standing) < 0.02, "{look:?} ends its blow standing");
            // Cut short as it winds up, and as it strikes
            for s in [0.3, 0.5, 0.6] {
                let mut last = pose(Some(blow_pose(look, s)));
                for k in 0..=480 {
                    let now = pose(Some(cut_pose(look, s, k as f64 / 480.0 * CUT_SHORT)));
                    let jump = crate::baked::furthest(&last, &now);
                    assert!(jump < 0.08, "{look:?} cut short {s} of the way through jumps {jump:.3} m at {k}");
                    last = now;
                }
                assert!(crate::baked::furthest(&last, &standing) < 0.02, "{look:?} cut short ends standing");
            }
        }
    }

    /// The frame it's drawn in further off is the one nearest how it's posed: standing, walking
    /// where it is in its stride, and the frames of its blow
    #[test]
    fn its_frame_is_the_nearest() {
        let standing = Doing::default();
        assert_eq!(nearest_frame(&standing), 0);
        let walking = Doing { phase: 0.5, stride: 1.0, ..Default::default() };
        assert_eq!(nearest_frame(&walking), animals::walk_frame(0.5));
        let wound = Doing { pose: Some(blow_pose(Look::Skeleton, WOUND)), ..Default::default() };
        assert_eq!(nearest_frame(&wound), POSE_WINDUP);
        let struck = Doing { pose: Some(blow_pose(Look::Skeleton, 0.6)), ..Default::default() };
        assert_eq!(nearest_frame(&struck), POSE_STRIKE);
    }
}
