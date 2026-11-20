//! Contact sheets of dragons moving, drawn without the game: each a run of frames of one dragon
//! going about one thing, its pieces' voxels splatted onto the page from the side or the front,
//! over a checkered ground so its feet can be seen planting. For judging how they move.
//! `DRAGON_SHEETS=dir cargo test --release dragon_sheets -- --ignored --nocapture`

use super::*;
use crate::animals::Animal;
use crate::bosses::Deed;

/// sRGB colors of the materials, as in the palette in shaders\traverse.comp
const PALETTE: [[f64; 3]; 90] = [
    [0.00, 0.00, 0.00], [0.28, 0.48, 0.17], [0.50, 0.35, 0.22], [0.58, 0.56, 0.55], [0.86, 0.74, 0.50], [0.95, 0.97, 1.00], [0.10, 0.30, 0.45], [0.45, 0.29, 0.17],
    [0.27, 0.54, 0.15], [0.92, 0.14, 0.14], [0.98, 0.84, 0.14], [0.80, 0.78, 0.74], [0.42, 0.56, 0.22], [0.10, 0.36, 0.10], [0.95, 0.75, 0.12], [0.95, 0.42, 0.06],
    [0.78, 0.08, 0.06], [0.58, 0.28, 0.85], [0.96, 0.96, 0.92], [0.68, 0.60, 0.54], [0.32, 0.48, 0.14], [0.90, 0.96, 0.98], [0.38, 0.63, 0.20], [0.10, 0.32, 0.20],
    [0.56, 0.53, 0.48], [0.15, 0.45, 0.95], [0.95, 0.45, 0.70], [0.40, 0.75, 0.20], [0.90, 0.88, 0.82], [0.98, 0.70, 0.80], [0.55, 0.75, 0.25], [0.85, 0.12, 0.10],
    [0.92, 0.88, 0.78], [0.55, 0.65, 0.30], [0.40, 0.25, 0.12], [0.30, 0.68, 0.15], [0.55, 0.42, 0.28], [0.45, 0.40, 0.36], [0.15, 0.12, 0.10], [0.36, 0.22, 0.12],
    [0.16, 0.10, 0.07], [0.80, 0.36, 0.12], [0.80, 0.60, 0.36], [0.52, 0.52, 0.54], [0.94, 0.92, 0.88], [0.03, 0.03, 0.03], [0.90, 0.50, 0.55], [0.92, 0.55, 0.20],
    [0.95, 0.85, 0.65], [0.88, 0.10, 0.10], [0.72, 0.24, 0.22], [0.95, 0.92, 0.84], [0.80, 0.72, 0.56], [0.50, 0.44, 0.36], [0.68, 0.20, 0.14], [0.16, 0.56, 0.54],
    [0.50, 0.95, 1.00], [1.00, 0.80, 0.45], [0.95, 0.90, 0.78], [0.88, 0.66, 0.44], [0.62, 0.42, 0.28], [0.68, 0.68, 0.64], [0.42, 0.42, 0.40], [0.86, 0.80, 0.68],
    [0.20, 0.58, 0.14], [0.80, 0.70, 0.40], [0.30, 0.56, 0.22], [0.18, 0.48, 0.10], [0.74, 0.42, 0.26], [0.80, 0.82, 0.86], [0.34, 0.36, 0.40], [0.98, 0.76, 0.26],
    [0.70, 0.44, 0.20], [0.40, 0.23, 0.13], [0.60, 0.10, 0.12], [0.86, 0.82, 0.70], [0.10, 0.08, 0.13], [0.92, 0.90, 0.82], [0.72, 0.88, 0.98], [1.00, 0.50, 0.12],
    [0.55, 0.85, 1.00], [0.70, 0.80, 1.00], [0.45, 1.00, 0.25], [0.80, 0.45, 1.00], [0.45, 0.12, 0.75], [0.95, 0.08, 0.12], [1.00, 0.92, 0.60], [0.62, 0.88, 0.16],
    [0.46, 0.03, 0.06], [0.58, 0.38, 0.27],
];

/// A pixel of a voxel of `material` facing `normal`, lit by a sun up to one side, in sRGB
fn shade(material: u8, normal: Vector3<f64>) -> [u8; 3] {
    let albedo = PALETTE[material as usize % PALETTE.len()].map(|c| c.powf(2.2));
    let emission = match material {
        FLAME => [6.0, 3.2, 1.2],
        CRYSTAL => [0.5, 1.6, 2.2],
        GLOW_FIRE..=GLOW_HOLY => albedo.map(|c| c * 3.0),
        _ => [0.0; 3],
    };
    let sun = Vector3::new(0.45, -0.55, 0.7).normalize();
    let lambert = normal.dot(&sun).max(0.0);
    let light = 0.5 + 2.1 * lambert;
    [0, 1, 2].map(|i| {
        let x = (albedo[i] * light + emission[i]) * 0.8;
        let aces = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
        (aces.powf(1.0 / 2.2) * 255.0) as u8
    })
}

/// A dragon's pieces' surface voxels, each in its piece's own meters with which way it faces
struct Splats {
    pieces: Vec<Vec<(Vector3<f64>, Vector3<f64>, u8)>>,
}

impl Splats {
    fn of(models: &[(Model, Vector3<f64>)], kind: usize) -> Splats {
        let pieces = (0..PIECES)
            .map(|m| {
                let (model, anchor) = &models[kind * PIECES + m];
                let [sx, sy, sz] = model.size.map(|v| v as i64);
                let at = |x: i64, y: i64, z: i64| if x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz { AIR } else { model.voxels[(x + (y + z * sy) * sx) as usize] };
                let mut out = Vec::new();
                for z in 0..sz {
                    for y in 0..sy {
                        for x in 0..sx {
                            let v = at(x, y, z);
                            if v == AIR {
                                continue;
                            }
                            let mut n = Vector3::zeros();
                            for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                                if at(x + dx, y + dy, z + dz) == AIR {
                                    n += Vector3::new(dx as f64, dy as f64, dz as f64);
                                }
                            }
                            if n.norm_squared() > 0.0 {
                                let p = anchor + Vector3::new(x as f64 + 0.5 - sx as f64 * 0.5, y as f64 + 0.5 - sy as f64 * 0.5, z as f64 + 0.5) / MODEL_RES;
                                out.push((p, n.normalize(), v));
                            }
                        }
                    }
                }
                out
            })
            .collect();
        Splats { pieces }
    }
}

/// Which way a frame's looked at the dragon from, and how much of it's in view
#[derive(Clone, Copy)]
struct Camera {
    target: Vector3<f64>,
    dir: Vector3<f64>,
    across: f64,
}

/// Dragon `d` drawn as `camera` sees it, `w` by `h` pixels, over checkered ground `ground` meters
/// up, `vpm` voxels to the meter where it is
fn draw(splats: &Splats, d: &Dragon, camera: Camera, ground: f64, vpm: f64, w: usize, h: usize) -> Vec<[u8; 3]> {
    let dir = camera.dir.normalize();
    let right = Vector3::new(dir.y, -dir.x, 0.0).try_normalize(1e-9).unwrap_or_else(Vector3::y);
    let up = right.cross(&dir);
    let scale = camera.across / w as f64;
    let mut color = vec![[0u8; 3]; w * h];
    let mut depth = vec![f64::MAX; w * h];
    // The sky and the ground, a meter's checks on it
    for j in 0..h {
        for i in 0..w {
            let (sx, sy) = ((i as f64 + 0.5 - w as f64 * 0.5) * scale, (h as f64 * 0.5 - j as f64 - 0.5) * scale);
            let o = camera.target + right * sx + up * sy - dir * 200.0;
            let sky = [(150.0 - 40.0 * j as f64 / h as f64) as u8, (175.0 - 30.0 * j as f64 / h as f64) as u8, 205];
            color[i + j * w] = sky;
            if dir.z < -1e-6 || (dir.z.abs() <= 1e-6 && o.z < ground) {
                let t = if dir.z < -1e-6 { (ground - o.z) / dir.z } else { 0.0 };
                let p = o + dir * t;
                if dir.z.abs() <= 1e-6 || t > 0.0 {
                    let check = ((p.x.floor() + p.y.floor()) as i64).rem_euclid(2) == 0;
                    color[i + j * w] = if check { [96, 110, 80] } else { [82, 95, 68] };
                    depth[i + j * w] = if dir.z.abs() <= 1e-6 { 1e9 } else { t - 200.0 };
                }
            }
        }
    }
    // Side on, the ground's a strip under the horizon
    if dir.z.abs() <= 1e-6 {
        for j in 0..h {
            let y = (h as f64 * 0.5 - j as f64 - 0.5) * scale + camera.target.z;
            if y < ground {
                for i in 0..w {
                    let p = camera.target + right * ((i as f64 + 0.5 - w as f64 * 0.5) * scale);
                    let check = ((p.x.floor() + p.y.floor()) as i64).rem_euclid(2) == 0;
                    color[i + j * w] = if check { [96, 110, 80] } else { [82, 95, 68] };
                }
            }
        }
    }
    let radius = ((1.0 / MODEL_RES) / scale * 0.7).max(0.6);
    let r = radius.ceil() as i64;
    for (b, bone) in d.bones.iter().enumerate() {
        if !bone.shown {
            continue;
        }
        for &(p, n, v) in &splats.pieces[model_of(b)] {
            let at = bone.at / vpm + bone.turn * p;
            let off = at - camera.target;
            let (x, y, z) = (off.dot(&right) / scale + w as f64 * 0.5, h as f64 * 0.5 - off.dot(&up) / scale, off.dot(&dir));
            let normal = bone.turn * n;
            // Facing away, it's behind what faces this way
            if normal.dot(&dir) > 0.35 {
                continue;
            }
            let c = shade(v, normal);
            for dy in -r..=r {
                for dx in -r..=r {
                    let (px, py) = (x as i64 + dx, y as i64 + dy);
                    if px < 0 || py < 0 || px >= w as i64 || py >= h as i64 {
                        continue;
                    }
                    let k = px as usize + py as usize * w;
                    if z < depth[k] {
                        depth[k] = z;
                        color[k] = c;
                    }
                }
            }
        }
    }
    color
}

/// Frames laid out in rows of `columns`, each `w` by `h`, a thin dark line between, saved to `path`
fn save(frames: &[Vec<[u8; 3]>], columns: usize, w: usize, h: usize, path: &std::path::Path) {
    let rows = frames.len().div_ceil(columns);
    let (width, height) = (columns * (w + 2) + 2, rows * (h + 2) + 2);
    let mut image = image::RgbImage::from_pixel(width as u32, height as u32, image::Rgb([20, 20, 24]));
    for (f, frame) in frames.iter().enumerate() {
        let (ox, oy) = (2 + (f % columns) * (w + 2), 2 + (f / columns) * (h + 2));
        for j in 0..h {
            for i in 0..w {
                image.put_pixel((ox + i) as u32, (oy + j) as u32, image::Rgb(frame[i + j * w]));
            }
        }
        // Its number, as a row of dots along the top, five to a bar
        for n in 0..=f {
            let x = ox + 3 + n * 4 + n / 5 * 3;
            if x + 2 < ox + w {
                for (a, b) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    image.put_pixel((x + a) as u32, (oy + 3 + b) as u32, image::Rgb([255, 240, 120]));
                }
            }
        }
    }
    image.save(path).expect("saving a contact sheet");
}

/// Flat open ground, as in the tests
fn open_ground(meters: u32, vpm: u32) -> World {
    let size = [meters * vpm, meters * vpm, 64 * vpm];
    let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
    let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
    occupancy[..tx * ty * (4 * vpm as usize / 8)].fill(u128::MAX);
    let mut world = World::from_packed_occupancy(size, &occupancy, STONE);
    world.voxels_per_meter = vpm;
    world
}

/// How a scene starts the dragon off
#[derive(Clone, Copy, PartialEq)]
enum Start {
    /// Standing about at home, nobody near
    Rest,
    /// After the player on foot
    Hunt,
    /// In the air after the player, circling
    Fly,
    /// Taking off from where it stands
    Rise,
    /// Coming in out of the air to land near the player
    Land,
}

/// One contact sheet: a dragon `look`, the player standing `away` meters off along x, started off
/// as `start`, making only move `only` if any, then `frames` frames `every` seconds apart from `at`
/// seconds in, looked at from `view` (side, front or quarter), following it or from where it began;
/// the player walking off at `flee` meters a second, and the dragon struck dead at `kill` seconds
struct Scene {
    name: String,
    look: Look,
    away: f64,
    start: Start,
    only: Option<Move>,
    at: f64,
    every: f64,
    frames: usize,
    view: &'static str,
    follow: bool,
    flee: f64,
    kill: Option<f64>,
    zoom: f64,
}

fn scene(name: &str, look: Look, start: Start, at: f64, every: f64, frames: usize) -> Scene {
    Scene { name: name.to_owned(), look, away: 8.0, start, only: None, at, every, frames, view: "side", follow: true, flee: 0.0, kill: None, zoom: 1.0 }
}

fn scenes() -> Vec<Scene> {
    let mut out = Vec::new();
    for look in KINDS {
        let n = format!("{look:?}").to_lowercase();
        let s = spec(look).s;
        let k = s.sqrt();
        // Walking after the player as they walk off, and running after them
        out.push(Scene { flee: speeds(look).0 * 0.9, away: 6.0 + 4.0 * s, ..scene(&format!("{n}_walk"), look, Start::Hunt, 2.5, 0.06 * k, 16) });
        out.push(Scene { flee: speeds(look).1 * 0.9, away: 10.0 + 6.0 * s, ..scene(&format!("{n}_run"), look, Start::Hunt, 2.5, 0.035 * k, 16) });
        // Standing about at home, breathing and looking round
        out.push(Scene { away: 90.0, view: "quarter", ..scene(&format!("{n}_idle"), look, Start::Rest, 1.0, 0.4, 16) });
        // Flying round the player, beating its wings and gliding between, from the side and the
        // front
        let beat = 1.0 / flight(look).beat;
        out.push(Scene { away: 40.0, flee: flight(look).cruise, ..scene(&format!("{n}_flight"), look, Start::Fly, 3.0, beat / 8.0, 24) });
        out.push(Scene { away: 40.0, flee: flight(look).cruise, view: "front", ..scene(&format!("{n}_flightfront"), look, Start::Fly, 3.0, beat / 8.0, 16) });
        out.push(Scene { away: 25.0, ..scene(&format!("{n}_glide"), look, Start::Fly, 2.0, 0.25, 24) });
        // Taking off, and coming down to land
        out.push(Scene { follow: false, zoom: 2.2, ..scene(&format!("{n}_takeoff"), look, Start::Rise, 0.0, 0.075 * k.sqrt(), 24) });
        out.push(Scene { zoom: 1.7, ..scene(&format!("{n}_landing"), look, Start::Land, 2.0, 0.13 * k.sqrt(), 28) });
        // Each of its moves on the ground, from as far off as it makes it from
        for m in [Bite, Tail, Breath, Gust, Stomp, Spit, Rain, Roar] {
            let Some(skill) = skills(look).iter().find(|sk| sk.m == m && !sk.air) else {
                continue;
            };
            let away = (skill.near + skill.far.min(12.0 * s)) * 0.5;
            let whole = skill.wind * k.powf(0.3) + if m == Breath { breath_length(look) } else { 1.0 } + skill.rest * k.powf(0.3);
            let view = if m == Tail { "quarter" } else { "side" };
            out.push(Scene { only: Some(m), away, view, follow: false, zoom: 1.15, ..scene(&format!("{n}_{}", format!("{m:?}").to_lowercase()), look, Start::Hunt, 0.4, (whole / 20.0).max(0.05), 24) });
        }
        // And in the air
        for m in [Swoop, Strafe, Dive, Hover, Spit, Rain] {
            if !skills(look).iter().any(|sk| sk.m == m && sk.air) {
                continue;
            }
            out.push(Scene { only: Some(m), away: 22.0, zoom: 1.3, ..scene(&format!("{n}_air{}", format!("{m:?}").to_lowercase()), look, Start::Fly, 2.0, 0.12, 24) });
        }
        // Struck dead on the ground, and in the air
        out.push(Scene { kill: Some(0.6), follow: false, zoom: 1.2, ..scene(&format!("{n}_death"), look, Start::Hunt, 0.5, 0.16, 24) });
        out.push(Scene { kill: Some(2.5), away: 25.0, follow: false, zoom: 2.2, ..scene(&format!("{n}_deathair"), look, Start::Fly, 2.4, 0.13, 24) });
    }
    out
}

/// Lays a scene out: the frames as they come
fn shoot(scene: &Scene, models: &[(Model, Vector3<f64>)]) -> Vec<Vec<[u8; 3]>> {
    let vpm = 4u32;
    let v = vpm as f64;
    let world = open_ground(400, vpm);
    let water = |_: f64, _: f64| f64::NEG_INFINITY;
    let mut feet = Vector3::new(200.0, 200.0, 4.0) * v;
    let home = feet - Vector3::new(scene.away, 0.0, 0.0) * v;
    let mut a = Animal::boss(scene.look, home, [1, 2, -1], 11);
    a.dragon.as_mut().unwrap().yaw = 0.0;
    let splats = Splats::of(models, kind(scene.look));
    let sp = spec(scene.look);
    let dt = 1.0 / 60.0;
    let (w, h) = (420usize, 300usize);
    let across = (reach(scene.look) * 1.3).max(5.0) * scene.zoom;
    // Once to set it down where it stands
    a.update(&world, &water, None, dt);
    if scene.start != Start::Rest {
        a.boss.as_mut().unwrap().provoke();
    }
    {
        let d = a.dragon.as_mut().unwrap();
        d.next = if scene.only.is_some() { 0.3 } else { 1e9 };
        d.budget = 1e9;
        match scene.start {
            Start::Hunt => d.state = State::Hunt,
            Start::Rise => d.state = State::Rise(0.0),
            Start::Fly | Start::Land => {
                let (from, up) = if scene.start == Start::Land { (home - Vector3::new(30.0 * v, 0.0, 0.0), 9.0) } else { (home, 10.0) };
                d.swoop_in(from + Vector3::new(0.0, 0.0, up * v), feet, v);
                d.next = if scene.only.is_some() { 0.6 } else { 1e9 };
                d.budget = if scene.start == Start::Land { 0.3 } else { 1e9 };
            }
            Start::Rest => {}
        }
    }
    let mut t = 0.0;
    let mut frames = Vec::new();
    let mut next = scene.at;
    let mut fixed: Option<Vector3<f64>> = None;
    while frames.len() < scene.frames && t < 60.0 {
        {
            let d = a.dragon.as_mut().unwrap();
            match scene.only {
                Some(m) => {
                    for (i, c) in d.cool.iter_mut().enumerate() {
                        if i != m as usize {
                            *c = 1e9;
                        }
                    }
                }
                None if d.state == State::Hunt => d.next = d.next.max(1.0),
                None => {}
            }
            if scene.start != Start::Land {
                d.budget = 1e9;
            }
            // Beating on and on, for its beat to be seen
            if scene.name.ends_with("_flight") || scene.name.ends_with("_flightfront") {
                d.glide = 0.0;
                d.beats = 10.0;
            }
        }
        if scene.kill.is_some_and(|when| t >= when) && a.health > 0.0 {
            a.damage(1e7, feet);
        }
        a.update(&world, &water, Some(feet), dt);
        a.boss.as_mut().unwrap().deeds.retain(|_: &Deed| false);
        feet.x += scene.flee * v * dt;
        t += dt;
        if t >= next {
            next += scene.every;
            let d = a.dragon.as_ref().unwrap();
            if std::env::var("DRAGON_DEBUG").is_ok() {
                println!("  off {:.1?} speed {:.1}", (d.center - feet) / v, d.velocity.norm() / v);
                println!(
                    "{} t {t:.2} {:?} air {} roll {:.2} pitch {:.2} yaw {:.2} shift {:.2?} neck {:.2?} head {:.2?} wings {:.2?} amp {:.2} beat {:.2} gait {:.2} pace {:.2} z {:.2} legs {:.2?}",
                    scene.name,
                    d.state,
                    d.airborne,
                    d.rig.roll,
                    d.rig.pitch,
                    d.yaw,
                    d.rig.shift,
                    d.rig.neck,
                    d.rig.head,
                    d.wings.base,
                    d.wings.amp,
                    d.beat,
                    d.gait,
                    d.pace,
                    (d.center.z - d.ground) / v,
                    d.rig.legs.map(|l| (l.swing, l.knee, l.plant))
                );
            }
            let center = d.center / v;
            let target = if scene.follow { Vector3::new(center.x, center.y, if scene.start == Start::Land { 4.0 + 1.8 * sp.s } else { center.z.max(4.0 + 1.0 * sp.s) }) } else { *fixed.get_or_insert(center + Vector3::new(0.0, 0.0, 0.4 * sp.s)) };
            let heading = if scene.follow { Vector3::new(d.yaw.cos(), d.yaw.sin(), 0.0) } else { Vector3::x() };
            let dir = match scene.view {
                // From its left side, square on
                "side" => Vector3::new(heading.y, -heading.x, 0.0),
                // From in front of it, a little above
                "front" => Vector3::new(-heading.x, -heading.y, -0.3),
                // From in front and to one side of it, from above
                _ => Vector3::new(heading.y - heading.x * 0.8, -heading.x - heading.y * 0.8, -0.5),
            };
            frames.push(draw(&splats, d, Camera { target, dir, across }, 4.0, v, w, h));
        }
    }
    frames
}

#[test]
#[ignore]
fn dragon_sheets() {
    let Ok(dir) = std::env::var("DRAGON_SHEETS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let models = build_models();
    let wanted: Vec<String> = std::env::var("DRAGON_SHEET").map_or(Vec::new(), |w| w.split(',').map(str::to_owned).collect());
    for scene in scenes() {
        if !wanted.is_empty() && !wanted.iter().any(|w| scene.name.contains(w.as_str())) {
            continue;
        }
        let frames = shoot(&scene, &models);
        save(&frames, 4, 420, 300, &dir.join(format!("{}.png", scene.name)));
        println!("{}: {} frames", scene.name, frames.len());
    }
}