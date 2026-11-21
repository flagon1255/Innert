//! Animations edited in the game's editor, see src\editor.rs, for any jointed rig: a dragon's, and
//! whatever else is made of pieces turned about joints. A rig says what its bones are and how the
//! code poses them, see Rig; an animation file, assets\anims\<rig>\<clip>.json, keyframes how far
//! each bone is turned and moved from where the code has it, eased from one key to the next, and
//! the game puts that on top of the code's pose as it draws the rig, see adjust(). So the code's
//! poses are always underneath, and a clip with no file is as the code has it.
//!
//! A rig plugs in with a few lines: give its bones, parents first, and call adjust() on its posed
//! joints before drawing them, see dragons::edited().

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use nalgebra::{UnitQuaternion, Vector3};
use serde::{Deserialize, Serialize};

use crate::animals::AnimalInstance;

/// A bone of a rig: what it's called, and the bone it hangs from
#[derive(Clone, Debug)]
pub struct BoneDef {
    pub name: String,
    pub parent: Option<usize>,
}

/// Where a bone's joint is and how it's turned, in the rig's own units from its middle
#[derive(Clone, Copy, Debug)]
pub struct Joint {
    pub at: Vector3<f64>,
    pub turn: UnitQuaternion<f64>,
    pub shown: bool,
}

/// A clip of a rig's: what it's called, how many seconds it lasts, and whether it goes round
#[derive(Clone, Debug)]
pub struct ClipDef {
    pub name: String,
    pub length: f64,
    pub looping: bool,
}

/// Something jointed the editor can show and animate, and the game draw with animations on top
pub trait Rig {
    /// What its files are kept under, assets\anims\<id>, as "dragon/ashwing"
    fn id(&self) -> String;
    /// Its bones, each after the one it hangs from
    fn bones(&self) -> Vec<BoneDef>;
    fn clips(&self) -> Vec<ClipDef>;
    /// Each bone's joint as the code poses it `t` seconds into clip `clip`, in meters from the
    /// rig's middle, facing +x
    fn pose(&self, clip: usize, t: f64) -> Vec<Joint>;
    /// What's drawn of it posed as `joints`, its middle at `at` in voxels of the world, facing `yaw`,
    /// bone `lit` lit up
    fn draw(&self, joints: &[Joint], at: Vector3<f64>, yaw: f64, vpm: f64, lit: Option<usize>, out: &mut Vec<AnimalInstance>);
}

/// How the way from one key to the next goes
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Ease {
    Linear,
    /// Slow out of the key and into the next
    #[default]
    Smooth,
    /// Slow out of the key, quick into the next
    In,
    /// Quick out of the key, slow into the next
    Out,
    /// Held till the next
    Step,
}

impl Ease {
    pub const ALL: [Ease; 5] = [Ease::Linear, Ease::Smooth, Ease::In, Ease::Out, Ease::Step];

    pub fn of(self, u: f64) -> f64 {
        let u = u.clamp(0.0, 1.0);
        match self {
            Ease::Linear => u,
            Ease::Smooth => u * u * (3.0 - 2.0 * u),
            Ease::In => u * u,
            Ease::Out => 1.0 - (1.0 - u) * (1.0 - u),
            Ease::Step => 0.0,
        }
    }
}

/// How far a bone's turned and moved from the code's pose at a moment of a clip: turned about its
/// own x, y and z, in degrees, then moved along them, in meters
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Key {
    pub time: f64,
    #[serde(default)]
    pub rotate: [f64; 3],
    #[serde(default)]
    pub offset: [f64; 3],
    #[serde(default)]
    pub ease: Ease,
}

/// A clip's file: each bone's keys, by its name
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Clip {
    #[serde(default)]
    pub rig: String,
    #[serde(default)]
    pub clip: String,
    pub length: f64,
    #[serde(default)]
    pub looping: bool,
    #[serde(default)]
    pub tracks: BTreeMap<String, Vec<Key>>,
}

impl Clip {
    /// How far bone `bone`'s turned and moved `t` seconds in, eased between its keys
    pub fn sample(&self, bone: &str, t: f64) -> Option<([f64; 3], [f64; 3])> {
        let keys = self.tracks.get(bone).filter(|k| !k.is_empty())?;
        let len = self.length.max(1e-3);
        let t = if self.looping { t.rem_euclid(len) } else { t.clamp(0.0, len) };
        let mix = |a: &Key, b: &Key, u: f64| {
            let e = a.ease.of(u);
            ([0, 1, 2].map(|i| a.rotate[i] + (b.rotate[i] - a.rotate[i]) * e), [0, 1, 2].map(|i| a.offset[i] + (b.offset[i] - a.offset[i]) * e))
        };
        let first = &keys[0];
        let last = &keys[keys.len() - 1];
        if keys.len() == 1 {
            return Some((first.rotate, first.offset));
        }
        if let Some(k) = keys.windows(2).find(|w| t >= w[0].time && t < w[1].time) {
            let u = (t - k[0].time) / (k[1].time - k[0].time).max(1e-9);
            return Some(mix(&k[0], &k[1], u));
        }
        if t >= last.time {
            // Round from the last key to the first again, for a clip that goes round
            if self.looping {
                let span = len - last.time + first.time;
                return Some(mix(last, first, (t - last.time) / span.max(1e-9)));
            }
            return Some((last.rotate, last.offset));
        }
        // Before the first key
        if self.looping {
            let span = len - last.time + first.time;
            return Some(mix(last, first, (t + len - last.time) / span.max(1e-9)));
        }
        Some((first.rotate, first.offset))
    }

    /// Keys bone `bone` as it is in `t` seconds, replacing a key already there
    pub fn set_key(&mut self, bone: &str, key: Key) {
        let keys = self.tracks.entry(bone.to_string()).or_default();
        keys.retain(|k| (k.time - key.time).abs() > 1e-4);
        keys.push(key);
        keys.sort_by(|a, b| a.time.total_cmp(&b.time));
    }

    pub fn remove_key(&mut self, bone: &str, time: f64) {
        if let Some(keys) = self.tracks.get_mut(bone) {
            keys.retain(|k| (k.time - time).abs() > 1e-4);
            if keys.is_empty() {
                self.tracks.remove(bone);
            }
        }
    }
}

/// A bone turned about its own x, y and z by `rotate` degrees
pub fn rotation(rotate: [f64; 3]) -> UnitQuaternion<f64> {
    let r = rotate.map(f64::to_radians);
    UnitQuaternion::from_axis_angle(&Vector3::z_axis(), r[2]) * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), r[1]) * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), r[0])
}

/// `joints`, posed by the code, with each bone turned and moved by `delta`, in its own frame, and
/// everything hanging from it carried with it. `meters` is how many of the joints' units make a meter.
pub fn apply_deltas(bones: &[BoneDef], joints: &mut [Joint], delta: &dyn Fn(usize) -> Option<([f64; 3], [f64; 3])>, meters: f64) {
    let code: Vec<Joint> = joints.to_vec();
    for b in 0..bones.len().min(joints.len()) {
        let (mut at, mut turn) = (code[b].at, code[b].turn);
        if let Some(p) = bones[b].parent.filter(|&p| p < b) {
            // Where it is from its parent as the code has it, kept from where its parent is now
            let inv = code[p].turn.inverse();
            at = joints[p].at + joints[p].turn * (inv * (code[b].at - code[p].at));
            turn = joints[p].turn * (inv * code[b].turn);
        }
        if let Some((r, o)) = delta(b) {
            turn *= rotation(r);
            at += turn * Vector3::from(o) * meters;
        }
        joints[b].at = at;
        joints[b].turn = turn;
    }
}

// The files

/// Where animation files are: assets\anims beside the game's sources
pub fn root() -> PathBuf {
    crate::assets::root().parent().map_or_else(|| PathBuf::from("anims"), |p| p.join("anims"))
}

pub fn file_of(rig: &str, clip: &str) -> PathBuf {
    root().join(rig).join(format!("{clip}.json"))
}

/// Whether there are any animation files, so a rig with none costs nothing to look for them
static ANY: AtomicBool = AtomicBool::new(false);
static CHECKED: AtomicBool = AtomicBool::new(false);

fn clips() -> std::sync::MutexGuard<'static, HashMap<(String, String), Option<Arc<Clip>>>> {
    static CLIPS: OnceLock<Mutex<HashMap<(String, String), Option<Arc<Clip>>>>> = OnceLock::new();
    CLIPS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// Forgets the files read, as when they've changed: they're read again as they're next wanted
pub fn forget() {
    clips().clear();
    CHECKED.store(false, Ordering::Relaxed);
}

/// Whether there are animation files to look in, so rigs need only ask for them when there are
pub fn active() -> bool {
    if !CHECKED.swap(true, Ordering::Relaxed) {
        let any = crate::assets::enabled() && std::fs::read_dir(root()).is_ok_and(|mut d| d.next().is_some());
        ANY.store(any, Ordering::Relaxed);
    }
    ANY.load(Ordering::Relaxed)
}

/// Clip `clip` of rig `rig` as its file has it, or the file for every rig of its kind, the part of
/// its id before the /, if it has none of its own
pub fn load(rig: &str, clip: &str) -> Option<Arc<Clip>> {
    if !active() {
        return None;
    }
    let key = (rig.to_string(), clip.to_string());
    if let Some(c) = clips().get(&key) {
        return c.clone();
    }
    let read = |rig: &str| -> Option<Clip> {
        let text = std::fs::read_to_string(file_of(rig, clip)).ok()?;
        match serde_json::from_str::<Clip>(&text) {
            Ok(c) => Some(c),
            Err(e) => {
                crate::assets::log(format!("{}: {e}", file_of(rig, clip).display()));
                None
            }
        }
    };
    let found = read(rig).or_else(|| rig.split_once('/').and_then(|(kind, _)| read(kind))).map(Arc::new);
    clips().insert(key, found.clone());
    found
}

pub fn save(rig: &str, clip: &Clip) -> Result<PathBuf, String> {
    let path = file_of(rig, &clip.clip);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut c = clip.clone();
    c.rig = rig.to_string();
    std::fs::write(&path, serde_json::to_string_pretty(&c).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    forget();
    Ok(path)
}

/// The game's hook: `joints` of rig `rig`, posed by the code `t` seconds into clip `clip`, with the
/// clip's file put on top, if it has one. Costs nothing while there are no animation files.
pub fn adjust(rig: &str, clip: &str, t: f64, bones: &dyn Fn() -> Vec<BoneDef>, joints: &mut [Joint], meters: f64) {
    let Some(c) = load(rig, clip) else { return };
    let bones = bones();
    apply_deltas(&bones, joints, &|b| c.sample(&bones[b].name, t), meters);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(time: f64, rotate: [f64; 3]) -> Key {
        Key { time, rotate, offset: [0.0; 3], ease: Ease::Linear }
    }

    /// Keys eased from one to the next, round from the last to the first in a clip that goes round
    #[test]
    fn keys_are_eased_between() {
        let mut c = Clip { length: 2.0, looping: true, ..Default::default() };
        c.set_key("head", key(0.0, [0.0, 0.0, 0.0]));
        c.set_key("head", key(1.0, [0.0, 90.0, 0.0]));
        assert_eq!(c.sample("head", 0.5).unwrap().0, [0.0, 45.0, 0.0]);
        assert_eq!(c.sample("head", 1.5).unwrap().0, [0.0, 45.0, 0.0], "on its way back to the first");
        assert_eq!(c.sample("head", 2.0).unwrap().0, [0.0, 0.0, 0.0]);
        assert!(c.sample("tail", 0.5).is_none());
        // A key set again at the same moment replaces it
        c.set_key("head", key(1.0, [10.0, 0.0, 0.0]));
        assert_eq!(c.tracks["head"].len(), 2);
    }

    /// A dragon's head keyed turned carries its jaw with it, and leaves its tail as the code has it
    #[test]
    fn a_dragon_takes_its_keys() {
        let rig = crate::dragons::EditorRig(crate::animals::Look::Ashwing);
        let bones = rig.bones();
        let code = rig.pose(0, 0.5);
        let mut joints = code.clone();
        let mut c = Clip { length: 4.0, looping: true, ..Default::default() };
        c.set_key("head", Key { time: 0.0, rotate: [0.0, 40.0, 0.0], offset: [0.0; 3], ease: Ease::Smooth });
        apply_deltas(&bones, &mut joints, &|b| c.sample(&bones[b].name, 0.5), 1.0);
        let at = |name: &str| bones.iter().position(|b| b.name == name).unwrap();
        assert!(joints[at("head")].turn.angle_to(&code[at("head")].turn) > 0.6);
        assert!((joints[at("jaw")].at - code[at("jaw")].at).norm() > 1e-3, "the jaw goes with the head");
        assert!((joints[at("tail_2")].at - code[at("tail_2")].at).norm() < 1e-12);
    }

    /// A bone turned carries what hangs from it round with it, about its own joint
    #[test]
    fn children_follow_their_bone() {
        let bones = vec![BoneDef { name: "neck".into(), parent: None }, BoneDef { name: "head".into(), parent: Some(0) }];
        let mut joints = vec![
            Joint { at: Vector3::zeros(), turn: UnitQuaternion::identity(), shown: true },
            Joint { at: Vector3::new(1.0, 0.0, 0.0), turn: UnitQuaternion::identity(), shown: true },
        ];
        apply_deltas(&bones, &mut joints, &|b| (b == 0).then_some(([0.0, 0.0, 90.0], [0.0; 3])), 1.0);
        assert!((joints[1].at - Vector3::new(0.0, 1.0, 0.0)).norm() < 1e-9, "{:?}", joints[1].at);
        assert!((joints[1].turn.angle() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }
}
