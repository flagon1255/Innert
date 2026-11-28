//! Jointed bosses drawn on the processor into contact sheets, for judging how they move without
//! the game: every voxel of every piece, where its pose puts it, splatted as a little square into
//! a picture seen from in front and to one side, shaded by which way it faces and with its shadow
//! on the ground under it. Only for tests, see tests::rig_sheets().

use std::path::Path;

use super::*;
use crate::bosses;

/// A frame of a reel: where each piece is, and how far through what it's doing, 0 to 1, with the
/// part of it, for the bar along its bottom
pub struct Frame {
    bones: [Bone; PIECES],
    through: f64,
    stage: Option<Stage>,
}

/// The colors of every material, read out of the shader, see PALETTE in shaders\traverse.comp
fn palette() -> Vec<[f64; 3]> {
    let text = include_str!("../../shaders/traverse.comp");
    let start = text.find("const vec3 PALETTE").expect("no palette in the shader");
    let body = &text[start..];
    let body = &body[body.find('(').unwrap() + 1..body.find(");").unwrap()];
    body.split("vec3(")
        .skip(1)
        .filter_map(|chunk| {
            let inside = &chunk[..chunk.find(')')?];
            let v: Vec<f64> = inside.split(',').filter_map(|n| n.trim().parse().ok()).collect();
            (v.len() == 3).then(|| [v[0], v[1], v[2]])
        })
        .collect()
}

fn glows(m: u8) -> bool {
    use crate::worldgen::material::*;
    matches!(m, CRYSTAL | FLAME | GLOW_FIRE | GLOW_FROST | GLOW_STORM | GLOW_POISON | GLOW_ARCANE | GLOW_VOID | GLOW_BLOOD | GLOW_HOLY | EYE)
}

/// Seconds between frames as reels are played
const DT: f64 = 1.0 / 30.0;

/// Plays `rig` through `seconds` of `moment` changing as `at` says, from 0 to 1, keeping a frame
/// every `every` of them
fn play(rig: &mut Rig, seconds: f64, frames: usize, mut at: impl FnMut(f64, &mut Moment)) -> Vec<Frame> {
    let steps = (seconds / DT).ceil() as usize;
    let mut out = Vec::new();
    let keep = |s: usize| frames <= 1 || s * (frames - 1) / steps.max(1) != (s + 1) * (frames - 1) / steps.max(1) || s == 0;
    for s in 0..=steps {
        let f = s as f64 / steps.max(1) as f64;
        let mut m = Moment::still(rig.look);
        m.dt = DT;
        m.engaged = true;
        m.target = Some(V::new(8.0, 0.0, 1.0));
        at(f, &mut m);
        rig.step(&m);
        if keep(s) && out.len() < frames {
            out.push(Frame { bones: rig.bones, through: f, stage: m.doing.map(|d| d.stage) });
        }
    }
    out
}

/// A move made all the way through, its windup, doing it and getting over it, as the boss makes it
fn the_move(rig: &mut Rig, m: Move, frames: usize) -> Vec<Frame> {
    let (wind, act, recover) = bosses::timing(rig.look, m);
    let whole = wind + act + recover + 0.4;
    let look = rig.look;
    play(rig, whole, frames, |f, moment| {
        let t = f * whole - 0.2;
        moment.doing = if t < 0.0 {
            None
        } else if t < wind {
            Some(Doing { m, stage: Stage::Wind, t, len: wind, held: 0.0 })
        } else if t < wind + act {
            Some(Doing { m, stage: Stage::Act, t: t - wind, len: act, held: 0.0 })
        } else if t < wind + act + recover {
            Some(Doing { m, stage: Stage::Recover, t: t - wind - act, len: recover, held: 0.0 })
        } else {
            None
        };
        // Aimed at the ground a little way ahead, near enough to be in the picture
        moment.aim = moment.doing.map(|_| V::new(3.0, 0.0, 0.0));
        if m == Move::Throw && look == Look::Colossus && (0.0..wind).contains(&t) {
            let tear = bosses::TEAR_PRY;
            moment.carry = Some(if t < tear { Carry::Prying(t / tear) } else { Carry::Held });
        }
    })
}

/// The reels to draw of `look`: walking, running, standing about, each move, hit and dying
pub fn reels(look: Look) -> Vec<(String, Vec<Frame>)> {
    let mut out = Vec::new();
    let mut rig = Rig::new(look, 11).unwrap();
    let stride = look.stride() / look.scale();
    for (name, speed) in [("walk", look.walk_speed()), ("run", look.run_speed())] {
        let mut phase = 0.0;
        // Into its stride first
        play(&mut rig, 1.5, 1, |_, m| {
            phase = (phase + speed * DT / stride).fract();
            (m.speed, m.phase) = (speed, phase);
        });
        let cycle = stride / speed;
        let frames = play(&mut rig, cycle, 16, |_, m| {
            phase = (phase + speed * DT / stride).fract();
            (m.speed, m.phase) = (speed, phase);
        });
        out.push((name.to_owned(), frames));
    }
    let frames = play(&mut rig, 6.0, 16, |_, m| m.engaged = false);
    out.push(("idle".to_owned(), frames));
    for m in moves(look) {
        let frames = the_move(&mut rig, m, 24);
        out.push((format!("{m:?}").to_lowercase(), frames));
    }
    // Struck hard from in front and then from the side, then reeling from it all
    let mut frames = Vec::new();
    for from in [V::new(1.0, 0.0, 0.0), V::new(0.2, 1.0, 0.0)] {
        rig.hit(0.04, from);
        frames.extend(play(&mut rig, 0.8, 8, |_, _| {}));
    }
    out.push(("hit".to_owned(), frames));
    let reel = bosses::REEL_SECONDS;
    let frames = play(&mut rig, reel + 0.4, 16, |f, m| {
        let t = f * (reel + 0.4);
        m.doing = (t < reel).then_some(Doing { m: Move::Roar, stage: Stage::Reel, t, len: reel, held: 0.0 });
    });
    out.push(("reel".to_owned(), frames));
    let death = death_seconds(look);
    let frames = play(&mut rig, death + 0.3, 24, |f, m| {
        m.dying = Some(f * (death + 0.3));
    });
    out.push(("death".to_owned(), frames));
    out
}

/// What `look` does in a fight, to draw each
fn moves(look: Look) -> Vec<Move> {
    bosses::repertoire(look)
}

/// One frame drawn, `size` pixels across: RGB, and the depth of each pixel
struct Picture {
    size: usize,
    color: Vec<[f64; 3]>,
    depth: Vec<f64>,
}

/// Draws `frames` of `look` in a grid, eight to a row, into `path`
pub fn sheet(look: Look, models: &[Model], frames: &[Frame], path: &Path) {
    let k = kind(look).unwrap();
    let size = 260;
    let palette = palette();
    let reach = look.reach() * 0.75;
    let columns = 8.min(frames.len().max(1));
    let rows = frames.len().div_ceil(columns).max(1);
    let mut image = image::RgbImage::new((columns * size) as u32, (rows * size) as u32);
    for (i, frame) in frames.iter().enumerate() {
        let picture = draw(look, k, models, frame, size, reach, &palette);
        let (ox, oy) = ((i % columns) * size, (i / columns) * size);
        for y in 0..size {
            for x in 0..size {
                let c = picture.color[x + y * size];
                let to = |v: f64| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
                image.put_pixel((ox + x) as u32, (oy + y) as u32, image::Rgb([to(c[0]), to(c[1]), to(c[2])]));
            }
        }
        // How far through it is along the bottom, colored by the part of the move
        let bar = match frame.stage {
            Some(Stage::Wind) => [230, 200, 60],
            Some(Stage::Act) => [230, 70, 50],
            Some(Stage::Recover) => [80, 140, 230],
            Some(Stage::Stun | Stage::Reel) => [190, 90, 220],
            None => [120, 120, 120],
        };
        let length = (frame.through * (size - 4) as f64) as usize;
        for x in 2..2 + length {
            for y in size - 6..size - 2 {
                image.put_pixel((ox + x) as u32, (oy + y) as u32, image::Rgb(bar));
            }
        }
    }
    image.save(path).expect("couldn't save the sheet");
}

fn draw(look: Look, k: usize, models: &[Model], frame: &Frame, size: usize, reach: f64, palette: &[[f64; 3]]) -> Picture {
    let mut p = Picture { size, color: vec![[0.0; 3]; size * size], depth: vec![f64::MAX; size * size] };
    // The sky, darker down to the ground
    for y in 0..size {
        let f = y as f64 / size as f64;
        for x in 0..size {
            p.color[x + y * size] = [0.32 - 0.12 * f, 0.36 - 0.12 * f, 0.42 - 0.1 * f];
        }
    }
    // Seen from in front, round to its left, and a little above
    let yaw = std::env::var("RIG_YAW").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0f64);
    let pitch = 0.22f64;
    let forward = V::new(-yaw.cos() * pitch.cos(), -yaw.sin() * pitch.cos(), -pitch.sin());
    let right = V::new(-yaw.sin(), yaw.cos(), 0.0);
    let up = right.cross(&forward);
    let scale = size as f64 / (2.2 * reach);
    let middle = V::new(0.0, 0.0, reach * 0.55);
    let project = |w: V| -> (f64, f64, f64) {
        let r = w - middle;
        (size as f64 * 0.5 + r.dot(&right) * scale, size as f64 * 0.55 - r.dot(&up) * scale, r.dot(&forward))
    };
    // The ground: a disc of grass under it
    for y in 0..size {
        for x in 0..size {
            // Back from the pixel to the ground
            let sx = (x as f64 - size as f64 * 0.5) / scale;
            let sy = -(y as f64 - size as f64 * 0.55) / scale;
            let o = middle + right * sx + up * sy;
            if forward.z.abs() > 1e-6 {
                let t = -o.z / forward.z;
                let g = o + forward * t;
                if g.xy().norm() < reach * 1.1 && t < p.depth[x + y * size] {
                    let checker = ((g.x.floor() + g.y.floor()) as i64).rem_euclid(2) as f64;
                    p.color[x + y * size] = [0.22 + 0.03 * checker, 0.34 + 0.03 * checker, 0.16];
                    p.depth[x + y * size] = t;
                }
            }
        }
    }
    let s = look.scale();
    let res = super::res(look);
    let splat = (scale / res * 1.25).ceil().max(1.0) as i64;
    let light = V::new(0.3, -0.5, 0.8).normalize();
    let layout = layout();
    let mut shadow = vec![false; size * size];
    let mut points: Vec<(V, V, u8)> = Vec::new();
    for (piece, bone) in frame.bones.iter().enumerate() {
        if !bone.shown {
            continue;
        }
        let (anchor, msize) = layout[k * PIECES + piece];
        if msize == [1, 1, 1] {
            continue;
        }
        let model = &models[k * PIECES + piece];
        let [sx, sy, sz] = model.size.map(|v| v as i64);
        let at = |x: i64, y: i64, z: i64| -> u8 {
            if x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz {
                AIR
            } else {
                model.voxels[(x + (y + z * sy) * sx) as usize]
            }
        };
        for z in 0..sz {
            for y in 0..sy {
                for x in 0..sx {
                    let m = at(x, y, z);
                    if m == AIR {
                        continue;
                    }
                    let mut normal = V::zeros();
                    for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                        if at(x + dx, y + dy, z + dz) == AIR {
                            normal += V::new(dx as f64, dy as f64, dz as f64);
                        }
                    }
                    if normal == V::zeros() {
                        continue;
                    }
                    let local = V::new((x as f64 + 0.5) / res - sx as f64 * 0.5 / res, (y as f64 + 0.5) / res - sy as f64 * 0.5 / res, (z as f64 + 0.5) / res) + anchor;
                    let world = bone.at * s + bone.turn * local;
                    points.push((world, bone.turn * normal.normalize(), m));
                }
            }
        }
    }
    // Shadows straight down on the ground first
    for &(w, _, _) in &points {
        let (px, py, _) = project(V::new(w.x - light.x * w.z / light.z, w.y - light.y * w.z / light.z, 0.0));
        for oy in 0..splat {
            for ox in 0..splat {
                let (x, y) = (px as i64 + ox, py as i64 + oy);
                if x >= 0 && y >= 0 && (x as usize) < size && (y as usize) < size {
                    shadow[x as usize + y as usize * size] = true;
                }
            }
        }
    }
    for (i, c) in p.color.iter_mut().enumerate() {
        if shadow[i] {
            *c = c.map(|v| v * 0.55);
        }
    }
    for &(w, n, m) in &points {
        let (px, py, depth) = project(w);
        let base = palette.get(m as usize).copied().unwrap_or([1.0, 0.0, 1.0]).map(|v: f64| v.powf(2.2));
        let shade = if glows(m) { 1.25 } else { 0.42 + 0.75 * n.dot(&light).max(0.0) + 0.12 * n.z.max(0.0) };
        let color = base.map(|v| v * shade);
        for oy in 0..splat {
            for ox in 0..splat {
                let (x, y) = (px as i64 + ox - splat / 2, py as i64 + oy - splat / 2);
                if x >= 0 && y >= 0 && (x as usize) < size && (y as usize) < size {
                    let i = x as usize + y as usize * size;
                    if depth < p.depth[i] {
                        p.depth[i] = depth;
                        p.color[i] = color;
                    }
                }
            }
        }
    }
    p
}
