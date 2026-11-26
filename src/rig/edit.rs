//! The jointed bosses as the model and animation editor sees them, see src\anim.rs and the editor:
//! their pieces' names, where the code anchors each, and their clips played as the code plays
//! them, with animation files laid on top in the game, see edited().

use super::*;
use crate::anim;
use crate::bosses;

/// What each of `look`'s pieces is called, by its index among its models, and the piece it hangs
/// from
pub fn bone_defs(look: Look) -> Vec<anim::BoneDef> {
    let named = |name: &str, parent: Option<usize>| anim::BoneDef { name: name.into(), parent };
    let mut out: Vec<anim::BoneDef> = match family(look) {
        Family::Beast => {
            let mut v = vec![named("body", None), named("neck", Some(0)), named("head", Some(1)), named("tail", Some(0))];
            for leg in ["fore_right", "fore_left", "hind_right", "hind_left"] {
                v.push(named(&format!("upper_{leg}"), Some(0)));
            }
            for (k, leg) in ["fore_right", "fore_left", "hind_right", "hind_left"].iter().enumerate() {
                v.push(named(&format!("lower_{leg}"), Some(4 + k)));
            }
            v
        }
        // Each hangs from the first, its heart or its bell, but a tendril's tip from the rest of it
        Family::Drifter => drifter::piece_names(look).iter().enumerate().map(|(p, (name, parent))| named(name, if p == 0 { None } else { Some(*parent) })).collect(),
        Family::Crawler => {
            let mut v = vec![named("body", None), named("abdomen", Some(0))];
            let legs: Vec<String> = (0..8).map(|k| format!("{}_{}", if k < 4 { "right" } else { "left" }, k % 4 + 1)).collect();
            for leg in &legs {
                v.push(named(&format!("femur_{leg}"), Some(0)));
            }
            for (k, leg) in legs.iter().enumerate() {
                v.push(named(&format!("tibia_{leg}"), Some(2 + k)));
            }
            v
        }
        _ => {
            use biped::*;
            let mut v: Vec<anim::BoneDef> = (0..EXTRA).map(|_| named("", None)).collect();
            v[PELVIS] = named("pelvis", None);
            v[CHEST] = named("chest", Some(PELVIS));
            v[HEAD] = named("head", Some(CHEST));
            v[JAW] = named("jaw", Some(HEAD));
            for (s, side) in ["right", "left"].iter().enumerate() {
                v[ARM[s]] = named(&format!("arm_{side}"), Some(CHEST));
                v[FORE[s]] = named(&format!("forearm_{side}"), Some(ARM[s]));
                v[THIGH[s]] = named(&format!("thigh_{side}"), Some(PELVIS));
                v[SHIN[s]] = named(&format!("shin_{side}"), Some(THIGH[s]));
                v[FOOT[s]] = named(&format!("foot_{side}"), Some(SHIN[s]));
            }
            v[TOOL] = named("held", Some(FORE[0]));
            v[CAPE] = named("cape", Some(CHEST));
            v
        }
    };
    // A build's own pieces, plates, roots, a boulder, hang from its middle
    while out.len() < PIECES {
        let k = out.len();
        out.push(named(&format!("extra_{}", k + 1), Some(0)));
    }
    out
}

/// What piece `p` of `look` is called, for its file
pub fn piece_name(look: Look, p: usize) -> String {
    bone_defs(look).get(p).map_or_else(|| format!("piece_{p:02}"), |b| b.name.clone())
}

/// Where each piece's model has the middle of its bottom from its joint, in meters, as the code
/// made it, see layout()
pub fn anchors() -> Vec<V> {
    layout().iter().map(|(a, _)| *a).collect()
}

/// A boss's clips: walking, running, standing about, each of its moves, reeling and dying
fn clip_names(look: Look) -> Vec<String> {
    let mut out: Vec<String> = ["idle", "walk", "run"].iter().map(|s| s.to_string()).collect();
    out.extend(bosses::repertoire(look).iter().map(|m| format!("{m:?}").to_lowercase()));
    out.push("reel".into());
    out.push("death".into());
    out
}

/// The clip it's in, and how far into it, as Moment `m` has it
fn clip_now(look: Look, m: &Moment, clock: f64) -> (String, f64) {
    if let Some(d) = m.dying {
        return ("death".into(), d);
    }
    if let Some(d) = m.doing {
        if d.stage == Stage::Reel || d.stage == Stage::Stun {
            return ("reel".into(), d.t);
        }
        let (wind, act, _) = bosses::timing(look, d.m);
        let t = match d.stage {
            Stage::Wind => d.t,
            Stage::Act => wind + d.t,
            _ => wind + act + d.t,
        };
        return (format!("{:?}", d.m).to_lowercase(), t);
    }
    let (walking, running) = m.pace();
    if running > 0.5 {
        ("run".into(), m.phase * m.stride / m.run.max(0.1))
    } else if walking > 0.2 {
        ("walk".into(), m.phase * m.stride / m.walk.max(0.1))
    } else {
        ("idle".into(), clock)
    }
}

/// Its pieces with the animation file of what it's doing laid on top, if there is one, see
/// src\anim.rs: called as it's posed, see Rig::step()
pub(super) fn edited(rig: &mut Rig, m: &Moment) {
    if !anim::active() {
        return;
    }
    let look = rig.look;
    let (clip, t) = clip_now(look, m, rig.clock);
    let mut joints: Vec<anim::Joint> = rig.bones.iter().map(|b| anim::Joint { at: b.at, turn: b.turn, shown: b.shown }).collect();
    anim::adjust(&EditorRig::id_of(look), &clip, t, &|| bone_defs(look), &mut joints, 1.0 / look.scale().max(1e-3));
    for (b, j) in rig.bones.iter_mut().zip(&joints) {
        b.at = j.at;
        b.turn = j.turn;
    }
}

/// A jointed boss's rig, for the editor and for animation files
pub struct EditorRig(pub Look);

impl EditorRig {
    fn id_of(look: Look) -> String {
        format!("boss/{}", crate::assets::snake(&format!("{look:?}")))
    }
}

/// Seconds between steps as a clip's played through for the editor
const DT: f64 = 1.0 / 30.0;

impl anim::Rig for EditorRig {
    fn id(&self) -> String {
        EditorRig::id_of(self.0)
    }

    fn bones(&self) -> Vec<anim::BoneDef> {
        bone_defs(self.0)
    }

    fn clips(&self) -> Vec<anim::ClipDef> {
        let look = self.0;
        let m = Moment::still(look);
        clip_names(look)
            .into_iter()
            .map(|name| {
                let (length, looping) = match name.as_str() {
                    "idle" => (6.0, true),
                    "walk" => (m.stride / m.walk.max(0.1), true),
                    "run" => (m.stride / m.run.max(0.1), true),
                    "reel" => (bosses::REEL_SECONDS + 0.4, false),
                    "death" => (death_seconds(look) + 0.3, false),
                    _ => {
                        let mv = bosses::repertoire(look).into_iter().find(|mv| format!("{mv:?}").to_lowercase() == name);
                        let (w, a, r) = mv.map_or((0.5, 0.5, 0.5), |mv| bosses::timing(look, mv));
                        (w + a + r + 0.3, false)
                    }
                };
                anim::ClipDef { name, length, looping }
            })
            .collect()
    }

    /// The code's own pose `t` seconds into clip `clip`: a boss made afresh and played through to
    /// there, as the game would, so its springs and easing come out as they do in a fight
    fn pose(&self, clip: usize, t: f64) -> Vec<anim::Joint> {
        let look = self.0;
        let names = clip_names(look);
        let name = names.get(clip).cloned().unwrap_or_default();
        let mut rig = Rig::new(look, 11).expect("a jointed boss");
        let moves = bosses::repertoire(look);
        let mv = moves.iter().copied().find(|mv| format!("{mv:?}").to_lowercase() == name);
        let steps = (t.max(0.0) / DT).ceil() as usize + 1;
        // Into its stride, so walking starts mid-walk as it's seen
        let lead = if matches!(name.as_str(), "walk" | "run") { 45 } else { 0 };
        for s in 0..steps + lead {
            let time = (s as f64 - lead as f64) * DT;
            let mut m = Moment::still(look);
            m.dt = DT;
            m.engaged = !matches!(name.as_str(), "idle");
            m.target = Some(V::new(8.0, 0.0, 1.0));
            match name.as_str() {
                "walk" | "run" => {
                    let speed = if name == "walk" { m.walk } else { m.run };
                    m.speed = speed;
                    m.phase = (time * speed / m.stride.max(0.1)).rem_euclid(1.0);
                }
                "reel" if time < bosses::REEL_SECONDS => {
                    m.doing = Some(Doing { m: bosses::Move::Roar, stage: Stage::Reel, t: time.max(0.0), len: bosses::REEL_SECONDS, held: 0.0 });
                }
                "death" => m.dying = Some(time.max(0.0)),
                _ => {
                    if let Some(mv) = mv {
                        let (wind, act, recover) = bosses::timing(look, mv);
                        m.doing = if time < 0.0 {
                            None
                        } else if time < wind {
                            Some(Doing { m: mv, stage: Stage::Wind, t: time, len: wind, held: 0.0 })
                        } else if time < wind + act {
                            Some(Doing { m: mv, stage: Stage::Act, t: time - wind, len: act, held: 0.0 })
                        } else if time < wind + act + recover {
                            Some(Doing { m: mv, stage: Stage::Recover, t: time - wind - act, len: recover, held: 0.0 })
                        } else {
                            None
                        };
                    }
                }
            }
            rig.step_code(&m);
        }
        rig.bones.iter().map(|b| anim::Joint { at: b.at, turn: b.turn, shown: b.shown }).collect()
    }

    fn draw(&self, joints: &[anim::Joint], at: V, yaw: f64, vpm: f64, lit: Option<usize>, out: &mut Vec<AnimalInstance>) {
        let look = self.0;
        let Some(k) = kind(look) else { return };
        let layout = layout();
        let base = animals::RIG_MODELS_AT + k * PIECES;
        let s = look.scale();
        let turn = rz(yaw);
        for (p, j) in joints.iter().enumerate().take(PIECES) {
            let (anchor, size) = layout[k * PIECES + p];
            if !j.shown || size == [1, 1, 1] {
                continue;
            }
            let [x, y, z] = size.map(|v| v as f64 / MODEL_RES);
            let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * vpm;
            let piece = turn * j.turn;
            let place = at + turn * j.at * s * vpm + piece * anchor * vpm;
            let mut instance = AnimalInstance::new((base + p) as u32, place, piece, vpm / MODEL_RES, reach, MOVING);
            if lit == Some(p) {
                instance = instance.skinned(0xA0_30_D0_FF);
            }
            out.push(instance);
        }
    }
}
