//! Painting the player's body, in the window a new character's made in, see creator.rs: a picture
//! of it big enough to paint voxel by voxel, turned round by dragging, zoomed with the wheel and
//! brought up close on the face, the head, the arms, the hands or the legs. What's painted is kept
//! as paint.rs keeps it, on the character.
//!
//! The picture's drawn on the processor, as the creator's turning one is, but face by face of the
//! voxels that show, so it knows which voxel's under the mouse, see render(). Painting makes only
//! the piece painted again, and draws the picture again; the mouse moving over it draws nothing
//! but the outline of what's under it.

use egui_winit_vulkano::egui::{self, Align2, Color32, Key, Modifiers, PointerButton, Pos2, RichText, Sense, Stroke, TextureOptions, pos2, vec2};
use nalgebra::{Matrix3, UnitQuaternion, Vector3};

use super::body::{self, BODY_RES, FOREARM, HEAD, LEG, PIECES, TORSO, UPPER_ARM};
use super::paint::{self, Cell, Colour, Paint, Role};
use super::{Character, Looks};
use crate::animals::Model;
use crate::worldgen::material::AIR;

type V = Vector3<f64>;
type Q = UnitQuaternion<f64>;

/// The ways out of a voxel through each of its faces, in the order faces are numbered
const NORMALS: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
/// The widest the picture's drawn, in pixels, however big it's shown, so drawing it stays quick
const MOST_PIXELS: f64 = 1100.0;

/// What part of the body to look at close up
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Whole,
    Head,
    Face,
    Torso,
    Arms,
    Hands,
    Legs,
}

impl Focus {
    pub const ALL: [Focus; 7] = [Focus::Face, Focus::Head, Focus::Whole, Focus::Torso, Focus::Arms, Focus::Hands, Focus::Legs];

    pub fn name(self) -> &'static str {
        match self {
            Focus::Whole => "Whole body",
            Focus::Head => "Head",
            Focus::Face => "Face",
            Focus::Torso => "Torso",
            Focus::Arms => "Arms",
            Focus::Hands => "Hands",
            Focus::Legs => "Legs",
        }
    }

    pub fn parse(name: &str) -> Option<Focus> {
        Focus::ALL.into_iter().find(|f| f.name().split(' ').next().is_some_and(|n| n.eq_ignore_ascii_case(name.trim())))
    }

    /// The pieces it's of, for putting them back as they were made
    fn pieces(self) -> Vec<usize> {
        match self {
            Focus::Whole => (0..PIECES).collect(),
            Focus::Head | Focus::Face => vec![HEAD],
            Focus::Torso => vec![TORSO],
            Focus::Arms => UPPER_ARM.into_iter().chain(FOREARM).collect(),
            Focus::Hands => FOREARM.to_vec(),
            Focus::Legs => LEG.to_vec(),
        }
    }
}

/// Where the picture's seen from: round `target`, `yaw` radians round from in front, `pitch` up
/// from level, `span` meters from its top to its bottom
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Camera {
    pub target: V,
    pub yaw: f64,
    pub pitch: f64,
    pub span: f64,
}

impl Camera {
    /// Which way's right, up and away in the picture
    fn basis(&self) -> (V, V, V) {
        let away = -V::new(self.yaw.cos() * self.pitch.cos(), self.yaw.sin() * self.pitch.cos(), self.pitch.sin());
        let right = V::new(-self.yaw.sin(), self.yaw.cos(), 0.0);
        (right, right.cross(&away), away)
    }
}

/// The body standing to be painted: as it stands in the world, its arms held a little out from
/// its sides, so their insides and the hands can be got at
pub fn pose(looks: &Looks) -> [(V, Q); PIECES] {
    let mut posed = body::pose(looks, &body::Stance { time: 1.0, ..Default::default() });
    for (k, side) in [-1.0f64, 1.0].into_iter().enumerate() {
        let spread = Q::from_axis_angle(&V::x_axis(), side * 0.32);
        let (shoulder, upper) = posed[UPPER_ARM[k]];
        posed[UPPER_ARM[k]].1 = spread * upper;
        let (elbow, fore) = posed[FOREARM[k]];
        posed[FOREARM[k]] = (shoulder + spread * (elbow - shoulder), spread * fore);
    }
    posed
}

/// Where to look from for `focus`, the body posed as `posed`
pub fn aim(focus: Focus, posed: &[(V, Q); PIECES]) -> Camera {
    let at = |p: usize, local: [f64; 3]| posed[p].0 + posed[p].1 * V::from(local);
    let (target, yaw, pitch, span) = match focus {
        Focus::Whole => (V::new(0.0, 0.0, 0.93), 0.35, 0.1, 2.05),
        Focus::Head => (at(HEAD, [-0.01, 0.0, 0.17]), 0.65, 0.15, 0.56),
        Focus::Face => (at(HEAD, [0.05, 0.0, 0.15]), 0.0, 0.04, 0.42),
        Focus::Torso => (at(TORSO, [0.0, 0.0, 0.26]), 0.3, 0.08, 0.92),
        Focus::Arms => (at(TORSO, [0.0, 0.0, 0.28]), 0.15, 0.06, 1.05),
        Focus::Hands => (at(FOREARM[0], [0.0, 0.0, -0.25]), -0.75, 0.15, 0.3),
        Focus::Legs => (at(TORSO, [0.0, 0.0, -0.42]), 0.3, 0.06, 1.12),
    };
    Camera { target, yaw, pitch, span }
}

/// A voxel's face, as it's shown under the mouse
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hit {
    pub piece: usize,
    /// Which voxel of the piece's model
    pub index: [i32; 3],
    /// Which of its faces, see NORMALS
    pub face: usize,
}

/// The picture of the body, and which voxel each of its pixels shows
pub struct Shot {
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
    /// For each pixel 0 for none, or the piece plus one, the face and the voxel, see id()
    ids: Vec<u32>,
    /// For each piece, from the corners of the voxels of its model to the picture: pixels across,
    /// down, and how far
    frames: [(Matrix3<f64>, V); PIECES],
    sizes: [[u32; 3]; PIECES],
}

fn id(p: usize, face: usize, index: u32) -> u32 {
    (p as u32 + 1) << 28 | (face as u32) << 25 | index
}

impl Shot {
    /// What's at pixel `x`, `y`
    pub fn hit(&self, x: f64, y: f64) -> Option<Hit> {
        if x < 0.0 || y < 0.0 || x >= self.size[0] as f64 || y >= self.size[1] as f64 {
            return None;
        }
        let v = self.ids[x as usize + y as usize * self.size[0]];
        if v == 0 {
            return None;
        }
        let piece = (v >> 28) as usize - 1;
        let [sx, sy, _] = self.sizes.get(piece)?.map(|s| s as i32);
        let i = (v & 0x1FF_FFFF) as i32;
        Some(Hit { piece, index: [i % sx, (i / sx) % sy, i / (sx * sy)], face: (v >> 25 & 7) as usize })
    }

    /// The corners of face `face` of voxel `index` of piece `p`, in the picture's pixels, round it
    pub fn corners(&self, p: usize, index: [i32; 3], face: usize) -> [[f64; 2]; 4] {
        let (a, b) = &self.frames[p];
        let (o, e1, e2) = face_frame(index, face);
        let at = |c: V| {
            let s = a * c + b;
            [s.x, s.y]
        };
        [at(o), at(o + e1), at(o + e1 + e2), at(o + e2)]
    }
}

/// Where face `face` of voxel `index` starts, in the corners of the model's voxels, and its two
/// sides
fn face_frame(index: [i32; 3], face: usize) -> (V, V, V) {
    let axis = face / 2;
    let mut o = V::new(index[0] as f64, index[1] as f64, index[2] as f64);
    if NORMALS[face][axis] > 0 {
        o[axis] += 1.0;
    }
    let unit = |a: usize| {
        let mut v = V::zeros();
        v[a] = 1.0;
        v
    };
    (o, unit((axis + 1) % 3), unit((axis + 2) % 3))
}

fn linear(rgb: [u8; 3]) -> [f64; 3] {
    rgb.map(|c| (c as f64 / 255.0).powf(2.2))
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// The body made of `models`, posed as `posed`, seen from `camera`, `size` pixels: each face of
/// each voxel that shows filled in, lit from over the eye's shoulder, edged when it's big enough
/// to see its edges
pub fn render(models: &[(Model, V)], looks: &Looks, posed: &[(V, Q); PIECES], camera: &Camera, size: [usize; 2]) -> Shot {
    let [w, h] = size;
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        let t = y as f64 / h as f64;
        for x in 0..w {
            let (dx, dy) = (x as f64 / w as f64 - 0.5, t - 0.45);
            let glow = (1.0 - (dx * dx * 1.2 + dy * dy).sqrt() * 1.5).max(0.0);
            let i = (x + y * w) * 4;
            rgba[i..i + 4].copy_from_slice(&[(14.0 + 22.0 * glow) as u8, (16.0 + 24.0 * glow) as u8, (22.0 + 30.0 * glow) as u8, 255]);
        }
    }
    let mut depth = vec![f64::MAX; w * h];
    let mut ids = vec![0u32; w * h];
    let (right, up, away) = camera.basis();
    let scale = h as f64 / camera.span;
    let light = (right * -0.45 + up * 0.65 - away * 0.6).normalize();
    let rows = Matrix3::from_rows(&[(right * scale).transpose(), (-up * scale).transpose(), away.transpose()]);
    let middle = V::new(w as f64 / 2.0, h as f64 / 2.0, 0.0);
    let mut frames = [(Matrix3::zeros(), V::zeros()); PIECES];
    let mut sizes = [[1u32; 3]; PIECES];
    let to = |v: f64| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
    for (p, &(joint, turn)) in posed.iter().enumerate() {
        let Some((model, anchor)) = models.get(p) else { continue };
        let [sx, sy, sz] = model.size.map(|v| v as i32);
        let a = rows * turn.to_rotation_matrix().matrix() / BODY_RES;
        let b = rows * (joint + turn * (anchor - V::new(sx as f64 / 2.0, sy as f64 / 2.0, 0.0) / BODY_RES) - camera.target) + middle;
        frames[p] = (a, b);
        sizes[p] = model.size;
        let normals: [V; 6] = NORMALS.map(|n| turn * V::new(n[0] as f64, n[1] as f64, n[2] as f64));
        let columns = [a.column(0).into_owned(), a.column(1).into_owned(), a.column(2).into_owned()];
        for z in 0..sz {
            for y in 0..sy {
                for x in 0..sx {
                    let index = (x + (y + z * sy) * sx) as usize;
                    let m = model.voxels[index];
                    if m == AIR {
                        continue;
                    }
                    let base = linear(paint::material_rgb(m, Some(p), looks));
                    let shade = 0.93 + 0.12 * (hash(index as u32 ^ (p as u32) << 24) & 0xFF) as f64 / 255.0;
                    for (face, n) in NORMALS.iter().enumerate() {
                        if normals[face].dot(&away) > -1e-6 || paint::voxel(model, [x + n[0], y + n[1], z + n[2]]) != AIR {
                            continue;
                        }
                        let axis = face / 2;
                        let (o, _, _) = face_frame([x, y, z], face);
                        let p0 = a * o + b;
                        let (e1, e2) = (columns[(axis + 1) % 3], columns[(axis + 2) % 3]);
                        let det = e1.x * e2.y - e1.y * e2.x;
                        if det.abs() < 1e-9 {
                            continue;
                        }
                        let xs = [p0.x, p0.x + e1.x, p0.x + e2.x, p0.x + e1.x + e2.x];
                        let ys = [p0.y, p0.y + e1.y, p0.y + e2.y, p0.y + e1.y + e2.y];
                        let x0 = xs.iter().copied().fold(f64::MAX, f64::min).floor().max(0.0) as usize;
                        let x1 = (xs.iter().copied().fold(f64::MIN, f64::max).ceil().max(0.0) as usize).min(w);
                        let y0 = ys.iter().copied().fold(f64::MAX, f64::min).floor().max(0.0) as usize;
                        let y1 = (ys.iter().copied().fold(f64::MIN, f64::max).ceil().max(0.0) as usize).min(h);
                        if x0 >= x1 || y0 >= y1 {
                            continue;
                        }
                        let lit = (0.36 + 0.62 * normals[face].dot(&light).max(0.0) + 0.1 * (-normals[face].dot(&away)).max(0.0)) * shade;
                        let colour = base.map(|c| to(c * lit));
                        let edge = base.map(|c| to(c * lit * 0.72));
                        let (l1, l2) = ((e1.x * e1.x + e1.y * e1.y).sqrt(), (e2.x * e2.x + e2.y * e2.y).sqrt());
                        let edged = l1.min(l2) > 7.0;
                        let face_id = id(p, face, index as u32);
                        for py in y0..y1 {
                            let dy = py as f64 + 0.5 - p0.y;
                            for px in x0..x1 {
                                let dx = px as f64 + 0.5 - p0.x;
                                let u = (dx * e2.y - dy * e2.x) / det;
                                let v = (e1.x * dy - e1.y * dx) / det;
                                if !(-0.002..=1.002).contains(&u) || !(-0.002..=1.002).contains(&v) {
                                    continue;
                                }
                                let d = p0.z + u * e1.z + v * e2.z;
                                let i = px + py * w;
                                if d >= depth[i] {
                                    continue;
                                }
                                depth[i] = d;
                                ids[i] = face_id;
                                let on_edge = edged && (u.min(1.0 - u) * l1 < 1.0 || v.min(1.0 - v) * l2 < 1.0);
                                let c = if on_edge { edge } else { colour };
                                rgba[i * 4..i * 4 + 3].copy_from_slice(&c);
                            }
                        }
                    }
                }
            }
        }
    }
    Shot { size, rgba, ids, frames, sizes }
}

/// What a click does
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    /// Paints the voxels under it in the colour chosen
    Paint,
    /// Adds voxels of the colour chosen onto the face under it, on the head
    Add,
    /// Takes away the voxels under it, on the head
    Remove,
    /// Takes the colour of the voxel under it to paint with
    Pick,
    /// Puts the voxels under it back as they were made
    Restore,
}

impl Tool {
    pub const ALL: [Tool; 5] = [Tool::Paint, Tool::Add, Tool::Remove, Tool::Pick, Tool::Restore];

    fn name(self) -> &'static str {
        match self {
            Tool::Paint => "Paint",
            Tool::Add => "Add",
            Tool::Remove => "Remove",
            Tool::Pick => "Pick",
            Tool::Restore => "Restore",
        }
    }

    fn about(self) -> &'static str {
        match self {
            Tool::Paint => "Paint the voxels you click or drag over. Left click.",
            Tool::Add => "Add voxels onto the face you click, for hair, a beard or horns. Head only, a little way past it. Shift + click.",
            Tool::Remove => "Take voxels away from the head. Ctrl + click.",
            Tool::Pick => "Take the colour of the voxel you click to paint with. Right click or Alt + click.",
            Tool::Restore => "Put the voxels you click back as they were made.",
        }
    }

    /// The colour its outline's drawn in
    fn outline(self) -> Color32 {
        match self {
            Tool::Paint => Color32::WHITE,
            Tool::Add => Color32::from_rgb(120, 240, 120),
            Tool::Remove => Color32::from_rgb(255, 110, 100),
            Tool::Pick => Color32::from_rgb(255, 220, 90),
            Tool::Restore => Color32::from_rgb(140, 200, 255),
        }
    }
}

/// One voxel changed by a stroke: its piece and where, what was painted there before and after
type Change = (usize, [i8; 3], Option<Cell>, Option<Cell>);

/// What's kept between frames while painting
pub struct Studio {
    pub focus: Focus,
    pub camera: Option<Camera>,
    pub colour: Colour,
    pub tool: Tool,
    /// A brush of 3 by 3 voxels over the surface, or one
    pub wide: bool,
    /// Painting the other side of the body too, as a face is mostly the same both sides
    pub mirror: bool,
    undo: Vec<Vec<Change>>,
    redo: Vec<Vec<Change>>,
    /// The stroke being painted, while the button's held, and what it does
    stroke: Option<(Tool, Vec<Change>)>,
    /// Turning the picture round with the left button, having pressed it off the body
    turning: bool,
    /// Where the stroke was last painted, in the picture's pixels
    last: Option<[f64; 2]>,
    /// The body as it's made for the look, before it's painted, what it is painted, and posed
    made_for: Option<Looks>,
    base: Vec<(Model, V)>,
    models: Vec<(Model, V)>,
    posed: [(V, Q); PIECES],
    revision: u64,
    shot: Option<(Shot, Camera, u64)>,
    texture: Option<egui::TextureHandle>,
    /// The mouse taken to be this far across and down the picture, for screenshot runs
    pub hover_at: Option<[f32; 2]>,
    /// Where the picture was last shown, on the screen
    shown: Option<egui::Rect>,
    /// What's said under the controls, as where the files were exported to
    said: Option<String>,
}

impl Studio {
    pub fn new(focus: Focus) -> Studio {
        Studio {
            focus,
            camera: None,
            colour: Colour::Material(paint::PAINT_FIRST + 60),
            tool: Tool::Paint,
            wide: false,
            mirror: true,
            undo: Vec::new(),
            redo: Vec::new(),
            stroke: None,
            turning: false,
            last: None,
            made_for: None,
            base: Vec::new(),
            models: Vec::new(),
            posed: [(V::zeros(), Q::identity()); PIECES],
            revision: 0,
            shot: None,
            texture: None,
            hover_at: None,
            shown: None,
            said: None,
        }
    }

    /// The body made again if the look's changed
    fn sync(&mut self, character: &Character) {
        if self.made_for == Some(character.looks) && self.models.len() >= PIECES {
            return;
        }
        let looks = character.looks;
        self.base = body::models(&looks);
        self.models = self.base.iter().map(|(m, a)| (crate::assets::copy(m), *a)).collect();
        character.paint.apply(&mut self.models);
        self.posed = pose(&looks);
        if self.camera.is_none() {
            self.camera = Some(aim(self.focus, &self.posed));
        }
        self.made_for = Some(looks);
        self.revision += 1;
    }

    /// Piece `p` painted again, alone
    fn repaint(&mut self, p: usize, paint: &Paint) {
        let (m, a) = &self.base[p];
        self.models[p] = (crate::assets::copy(m), *a);
        paint.apply_piece(p, &mut self.models[p]);
        self.revision += 1;
    }

    fn voxel_at(&self, p: usize, key: [i8; 3]) -> (u8, u8) {
        let at = |(m, a): &(Model, V)| paint::voxel(m, paint::index_of(m, a, key));
        (at(&self.base[p]), at(&self.models[p]))
    }

    /// The voxels a stroke at `hit` touches: the one hit, or those round it over the surface, and
    /// mirrored, those on the other side; each with the face of it that's towards the eye, and
    /// whether it's on the other side
    pub fn brush(&self, hit: &Hit) -> Vec<(usize, [i8; 3], usize, bool)> {
        let p = hit.piece;
        let (m, a) = &self.models[p];
        let n = NORMALS[hit.face];
        let axis = hit.face / 2;
        let step = |c: [i32; 3], k: i32| [c[0] + n[0] * k, c[1] + n[1] * k, c[2] + n[2] * k];
        let r = if self.wide { 1 } else { 0 };
        let mut out = Vec::new();
        for du in -r..=r {
            for dv in -r..=r {
                let mut c = hit.index;
                c[(axis + 1) % 3] += du;
                c[(axis + 2) % 3] += dv;
                // The voxel showing on the surface there: on the layer hit, or one out or in
                let surface = [c, step(c, 1), step(c, -1)].into_iter().find(|&c| paint::voxel(m, c) != AIR && paint::voxel(m, step(c, 1)) == AIR);
                if let Some(key) = surface.and_then(|c| paint::key_of(m, a, c)) {
                    out.push((p, key, hit.face, false));
                }
            }
        }
        if self.mirror {
            let flip = |f: usize| if f / 2 == 1 { f ^ 1 } else { f };
            let mirrored: Vec<_> = out.iter().filter_map(|&(p, k, f, _)| paint::mirrored(&self.base, p, k).map(|(q, j)| (q, j, flip(f), true))).collect();
            for t in mirrored {
                if !out.iter().any(|o| o.0 == t.0 && o.1 == t.1) {
                    out.push(t);
                }
            }
        }
        out
    }

    /// What `tool` does at `hit`, kept in the stroke being painted
    fn stroke_at(&mut self, paint: &mut Paint, looks: &Looks, hit: &Hit, tool: Tool) {
        let mut touched = [false; PIECES];
        for (p, key, face, _) in self.brush(hit) {
            let n = NORMALS[face];
            let key = if tool == Tool::Add {
                let k = [0, 1, 2].map(|a| key[a] as i32 + n[a]);
                match k.map(i8::try_from) {
                    [Ok(x), Ok(y), Ok(z)] => [x, y, z],
                    _ => continue,
                }
            } else {
                key
            };
            let (base, now) = self.voxel_at(p, key);
            let before = paint.get(p, key);
            let m = self.colour.material(p, looks);
            let after = match tool {
                Tool::Paint if now != AIR => {
                    if before.is_some_and(|c| c.added) {
                        Some(Cell { voxel: m, added: true })
                    } else {
                        (m != base).then_some(Cell { voxel: m, added: false })
                    }
                }
                Tool::Add if now == AIR && m != AIR && paint::has_room(p, &self.base[p], key) => {
                    if base != AIR { (m != base).then_some(Cell { voxel: m, added: false }) } else { Some(Cell { voxel: m, added: true }) }
                }
                Tool::Remove if now != AIR && p == HEAD => (base != AIR).then_some(Cell { voxel: AIR, added: false }),
                Tool::Restore => None,
                _ => continue,
            };
            if after == before {
                continue;
            }
            if paint.set(p, key, after) != before {
                // Full up, see paint::MOST
                continue;
            }
            if let Some((_, changes)) = &mut self.stroke {
                match changes.iter_mut().find(|c| c.0 == p && c.1 == key) {
                    Some(c) => c.3 = after,
                    None => changes.push((p, key, before, after)),
                }
            }
            touched[p] = true;
        }
        for p in (0..PIECES).filter(|&p| touched[p]) {
            self.repaint(p, paint);
        }
    }

    /// The stroke finished, to be undone
    fn finish(&mut self) {
        if let Some((_, changes)) = self.stroke.take()
            && !changes.is_empty()
        {
            self.undo.push(changes);
            self.redo.clear();
        }
        self.last = None;
    }

    fn put(&mut self, paint: &mut Paint, changes: &[Change], undoing: bool) {
        let mut touched = [false; PIECES];
        let mut set = |c: &Change| {
            paint.set(c.0, c.1, if undoing { c.2 } else { c.3 });
            touched[c.0] = true;
        };
        if undoing {
            changes.iter().rev().for_each(&mut set);
        } else {
            changes.iter().for_each(&mut set);
        }
        for p in (0..PIECES).filter(|&p| touched[p]) {
            self.repaint(p, paint);
        }
    }

    pub fn undo(&mut self, paint: &mut Paint) {
        self.finish();
        if let Some(changes) = self.undo.pop() {
            self.put(paint, &changes, true);
            self.redo.push(changes);
        }
    }

    pub fn redo(&mut self, paint: &mut Paint) {
        self.finish();
        if let Some(changes) = self.redo.pop() {
            self.put(paint, &changes, false);
            self.undo.push(changes);
        }
    }

    /// Everything painted on `pieces` taken off, to be undone
    pub fn reset(&mut self, paint: &mut Paint, pieces: &[usize]) {
        self.finish();
        let changes: Vec<Change> = pieces.iter().flat_map(|&p| paint.on(p).map(move |(k, c)| (p, k, Some(c), None))).collect();
        if !changes.is_empty() {
            self.put(paint, &changes, false);
            self.undo.push(changes);
            self.redo.clear();
        }
    }

    /// Painting `tool` at `hit` as a single click, for tests
    #[cfg(test)]
    pub fn click(&mut self, character: &mut Character, hit: &Hit, tool: Tool) {
        self.sync(character);
        self.stroke = Some((tool, Vec::new()));
        let looks = character.looks;
        self.stroke_at(&mut character.paint, &looks, hit, tool);
        self.finish();
    }

    /// The tool a click uses, held down with `modifiers`
    fn tool_for(&self, modifiers: Modifiers) -> Tool {
        if modifiers.alt {
            Tool::Pick
        } else if modifiers.shift {
            Tool::Add
        } else if modifiers.command {
            Tool::Remove
        } else {
            self.tool
        }
    }

    /// The picture as it is now, drawn again if anything's changed
    fn shot(&mut self, looks: &Looks, size: [usize; 2]) -> &Shot {
        let camera = self.camera.unwrap_or_else(|| aim(self.focus, &self.posed));
        let fresh = self.shot.as_ref().is_some_and(|(s, c, r)| s.size == size && *c == camera && *r == self.revision);
        if !fresh {
            self.shot = Some((render(&self.models, looks, &self.posed, &camera, size), camera, self.revision));
        }
        &self.shot.as_ref().unwrap().0
    }

    fn look_at(&mut self, focus: Focus) {
        self.focus = focus;
        self.camera = Some(aim(focus, &self.posed));
    }
}

fn color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// The painting, in place of the rest of the window a new character's made in: true once it's
/// done with, to go back to the rest
pub fn window(ctx: &egui::Context, studio: &mut Studio, character: &mut Character) -> bool {
    studio.sync(character);
    let mut done = false;
    let screen = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::background()).rect_filled(screen, 0.0, Color32::from_black_alpha(150));
    let (undo, redo) = ctx.input_mut(|i| {
        let redo = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y);
        (i.consume_key(Modifiers::COMMAND, Key::Z), redo)
    });
    if undo {
        studio.undo(&mut character.paint);
    }
    if redo {
        studio.redo(&mut character.paint);
    }
    let margin = 24.0;
    let wide = (screen.width() - 2.0 * margin).clamp(320.0, 1500.0);
    let tall = (screen.height() - 2.0 * margin).max(260.0);
    egui::Area::new(egui::Id::new("character painting")).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        crate::theme::framed(ui, crate::theme::panel(20.0, 16.0), |ui| {
            ui.set_width(wide - 40.0);
            ui.set_max_height(tall - 40.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Paint your character").font(crate::theme::display(25.0)).color(crate::theme::HEADING));
                ui.add_space(10.0);
                ui.label(RichText::new("voxel by voxel: war paint, scars, tattoos, a beard. Changing your looks after keeps what's painted.").size(14.0).color(crate::theme::WEAK));
            });
            ui.add_space(6.0);
            let footer = 50.0;
            let side = 300.0;
            let view_height = (tall - 40.0 - 42.0 - footer).max(150.0);
            let view_width = (wide - 40.0 - side - 16.0).max(200.0);
            ui.horizontal_top(|ui| {
                view(ui, studio, character, [view_width, view_height]);
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    ui.set_width(side);
                    ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                    egui::ScrollArea::vertical().max_height(view_height).auto_shrink([false, false]).show(ui, |ui| controls(ui, studio, character));
                });
            });
            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_sized([130.0, 36.0], egui::Button::new(RichText::new("Done").font(crate::theme::display(18.0)).color(Color32::WHITE)).fill(crate::theme::PRIMARY)).clicked() {
                    done = true;
                }
                ui.add_space(10.0);
                ui.add(egui::Label::new(RichText::new("Left: paint   Right click or Alt: pick a colour   Shift: add   Ctrl: remove   Right drag: turn   Middle drag: move   Wheel: zoom   Ctrl+Z / Ctrl+Y: undo / redo").size(12.5).color(Color32::from_gray(170))).wrap());
            });
        });
    });
    if done {
        studio.finish();
    }
    done
}

/// The picture to paint on, `size` across and down
fn view(ui: &mut egui::Ui, studio: &mut Studio, character: &mut Character, size: [f32; 2]) {
    let (rect, response) = ui.allocate_exact_size(vec2(size[0], size[1]), Sense::click_and_drag());
    studio.shown = Some(rect);
    let k = (ui.ctx().pixels_per_point() as f64).min(MOST_PIXELS / size[0].max(size[1]) as f64);
    let pixels = [(size[0] as f64 * k).round().max(8.0) as usize, (size[1] as f64 * k).round().max(8.0) as usize];
    let looks = character.looks;
    let to_pixels = |p: Pos2| [((p.x - rect.min.x) / rect.width()) as f64 * pixels[0] as f64, ((p.y - rect.min.y) / rect.height()) as f64 * pixels[1] as f64];
    let pointer = studio.hover_at.map(|[x, y]| rect.min + vec2(x * rect.width(), y * rect.height())).or_else(|| response.hover_pos());
    let (pressed, down, modifiers, scroll) = ui.ctx().input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.modifiers, i.smooth_scroll_delta.y));

    // Painting: pressed on the body, painting every voxel the mouse goes over till it's let go,
    // along the way from where it was, so a quick stroke leaves no gaps. Pressed off the body,
    // the left button turns it round.
    if pressed && response.hovered() {
        let shot = studio.shot(&looks, pixels);
        let hit = pointer.and_then(|p| {
            let [x, y] = to_pixels(p);
            shot.hit(x, y)
        });
        let tool = studio.tool_for(modifiers);
        match hit {
            Some(h) if tool == Tool::Pick => studio.colour = Colour::of(paint::voxel(&studio.models[h.piece].0, h.index), h.piece),
            Some(_) => {
                studio.stroke = Some((tool, Vec::new()));
                studio.last = None;
            }
            None => studio.turning = true,
        }
    }
    if let (Some(tool), Some(p), true) = (studio.stroke.as_ref().map(|s| s.0), pointer, down) {
        let to = to_pixels(p);
        let from = studio.last.unwrap_or(to);
        let steps = (((to[0] - from[0]).powi(2) + (to[1] - from[1]).powi(2)).sqrt() / 2.0).ceil().max(1.0) as usize;
        let mut hits = Vec::new();
        {
            let shot = studio.shot(&looks, pixels);
            for s in 1..=steps {
                let t = s as f64 / steps as f64;
                if let Some(h) = shot.hit(from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t)
                    && hits.last() != Some(&h)
                {
                    hits.push(h);
                }
            }
        }
        for h in hits {
            studio.stroke_at(&mut character.paint, &looks, &h, tool);
        }
        studio.last = Some(to);
    }
    if !down {
        studio.finish();
        studio.turning = false;
    }
    // Turning round with the right button, or the left pressed off the body; moving it about
    // with the middle; zooming with the wheel
    if let Some(camera) = &mut studio.camera {
        let delta = response.drag_delta();
        let span = camera.span;
        if response.dragged_by(PointerButton::Secondary) || (studio.turning && response.dragged_by(PointerButton::Primary)) {
            camera.yaw -= delta.x as f64 * 0.01;
            camera.pitch = (camera.pitch + delta.y as f64 * 0.01).clamp(-1.3, 1.4);
        }
        if response.dragged_by(PointerButton::Middle) {
            let (right, up, _) = camera.basis();
            camera.target += (-right * delta.x as f64 + up * delta.y as f64) * span / rect.height() as f64;
        }
        if response.hovered() && scroll != 0.0 {
            camera.span = (span * (-scroll as f64 * 0.0015).exp()).clamp(0.12, 3.0);
        }
    }
    let shot = studio.shot(&looks, pixels);
    let hit = pointer.and_then(|p| {
        let [x, y] = to_pixels(p);
        shot.hit(x, y)
    });
    if response.clicked_by(PointerButton::Secondary)
        && let Some(h) = hit
    {
        studio.colour = Colour::of(paint::voxel(&studio.models[h.piece].0, h.index), h.piece);
    }

    // The picture, and what's under the mouse outlined
    let shot = studio.shot(&looks, pixels);
    let image = egui::ColorImage::from_rgba_unmultiplied(shot.size, &shot.rgba);
    match &mut studio.texture {
        Some(t) => t.set(image, TextureOptions::NEAREST),
        None => studio.texture = Some(ui.ctx().load_texture("character painting", image, TextureOptions::NEAREST)),
    }
    let painter = ui.painter_at(rect);
    if let Some(t) = &studio.texture {
        painter.image(t.id(), rect, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    painter.rect_stroke(rect, 8.0, Stroke::new(1.0f32, crate::theme::BRONZE.gamma_multiply(0.7)), egui::StrokeKind::Inside);
    let tool = studio.stroke.as_ref().map_or_else(|| studio.tool_for(modifiers), |s| s.0);
    if let Some(h) = hit {
        let shot = &studio.shot.as_ref().unwrap().0;
        let to_screen = |c: [f64; 2]| rect.min + vec2((c[0] / pixels[0] as f64) as f32 * rect.width(), (c[1] / pixels[1] as f64) as f32 * rect.height());
        let targets = if tool == Tool::Pick { vec![(h.piece, [0; 3], h.face, false)] } else { studio.brush(&h) };
        for &(p, key, face, mirrored) in &targets {
            let index = if tool == Tool::Pick { h.index } else { paint::index_of(&studio.models[p].0, &studio.models[p].1, key) };
            let corners: Vec<Pos2> = shot.corners(p, index, face).iter().map(|&c| to_screen(c)).collect();
            // Grey where the tool does nothing, as adding or taking away off the head
            let idle = matches!(tool, Tool::Add | Tool::Remove) && p != HEAD;
            let colour = if idle { Color32::from_gray(120) } else if mirrored { tool.outline().gamma_multiply(0.55) } else { tool.outline() };
            painter.add(egui::Shape::closed_line(corners, Stroke::new(if mirrored { 1.5f32 } else { 2.0 }, colour)));
        }
        let m = paint::voxel(&studio.models[h.piece].0, h.index);
        let what = format!("{} · {}", body::PIECE_NAMES[h.piece].replace('_', " "), Colour::of(m, h.piece).name());
        painter.text(rect.left_bottom() + vec2(10.0, -10.0), Align2::LEFT_BOTTOM, what, egui::FontId::proportional(13.0), Color32::from_gray(200));
    }
    if response.hovered() || studio.stroke.is_some() {
        ui.ctx().set_cursor_icon(if hit.is_some() { egui::CursorIcon::Crosshair } else { egui::CursorIcon::Grab });
    }
}

/// A swatch of `rgb` to click, ringed when it's chosen
fn swatch(ui: &mut egui::Ui, rgb: [u8; 3], chosen: bool, name: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
    ui.painter().rect_filled(rect.shrink(2.0), 4.0, color(rgb));
    ui.painter().rect_stroke(rect.shrink(1.0), 5.0, Stroke::new(if chosen { 2.5f32 } else { 1.0 }, if chosen { Color32::WHITE } else { Color32::from_gray(70) }), egui::StrokeKind::Inside);
    response.on_hover_text(name).clicked()
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(8.0);
    crate::theme::heading(ui, text, 16.0);
    ui.add_space(2.0);
}

/// Everything to choose how to paint with, beside the picture
fn controls(ui: &mut egui::Ui, studio: &mut Studio, character: &mut Character) {
    let looks = character.looks;
    heading(ui, "Look at");
    ui.horizontal_wrapped(|ui| {
        for focus in Focus::ALL {
            if ui.selectable_label(studio.focus == focus, RichText::new(focus.name()).size(14.0)).clicked() {
                studio.look_at(focus);
            }
        }
    });
    ui.horizontal(|ui| {
        if let Some(camera) = &mut studio.camera {
            if ui.button(RichText::new("⟲").size(15.0)).on_hover_text("Turn left").clicked() {
                camera.yaw += 0.4;
            }
            if ui.button(RichText::new("⟳").size(15.0)).on_hover_text("Turn right").clicked() {
                camera.yaw -= 0.4;
            }
            if ui.button(RichText::new(" + ").size(15.0)).on_hover_text("Closer").clicked() {
                camera.span = (camera.span / 1.25).max(0.12);
            }
            if ui.button(RichText::new(" − ").size(15.0)).on_hover_text("Further").clicked() {
                camera.span = (camera.span * 1.25).min(3.0);
            }
        }
    });

    heading(ui, "Tool");
    ui.horizontal_wrapped(|ui| {
        for tool in Tool::ALL {
            if ui.selectable_label(studio.tool == tool, RichText::new(tool.name()).size(14.0)).on_hover_text(tool.about()).clicked() {
                studio.tool = tool;
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("Brush").size(14.0).color(Color32::from_gray(200)));
        ui.selectable_value(&mut studio.wide, false, RichText::new("1 voxel").size(14.0));
        ui.selectable_value(&mut studio.wide, true, RichText::new("3 × 3").size(14.0));
    });
    ui.checkbox(&mut studio.mirror, RichText::new("Mirror left and right").size(14.0));

    heading(ui, "Colour");
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(40.0, 26.0), Sense::hover());
        ui.painter().rect_filled(rect, 5.0, color(studio.colour.rgb(&looks)));
        ui.painter().rect_stroke(rect, 5.0, Stroke::new(1.5f32, Color32::WHITE), egui::StrokeKind::Inside);
        ui.label(RichText::new(studio.colour.name()).size(13.5).color(Color32::from_gray(220)));
    });
    ui.label(RichText::new("Your looks' colours").size(13.0).color(Color32::from_gray(180)));
    ui.horizontal_wrapped(|ui| {
        for role in Role::ALL {
            let c = Colour::Look(role);
            if swatch(ui, role.rgb(&looks), studio.colour == c, &format!("{}: follows your looks where it's worn", role.name())) {
                studio.colour = c;
            }
        }
    });
    ui.label(RichText::new("Paints").size(13.0).color(Color32::from_gray(180)));
    ui.horizontal_wrapped(|ui| {
        for m in paint::paints().skip(paint::PAINT_COUNT - paint::EXTRAS.len()) {
            let (name, rgb) = paint::paint(m).unwrap_or(("", [0; 3]));
            if swatch(ui, rgb, studio.colour == Colour::Material(m), name) {
                studio.colour = Colour::Material(m);
            }
        }
    });
    egui::CollapsingHeader::new(RichText::new("Every skin, hair and cloth").size(13.0)).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for m in paint::paints().take(paint::PAINT_COUNT - paint::EXTRAS.len()) {
                let (name, rgb) = paint::paint(m).unwrap_or(("", [0; 3]));
                if swatch(ui, rgb, studio.colour == Colour::Material(m), name) {
                    studio.colour = Colour::Material(m);
                }
            }
        });
    });

    heading(ui, "Changes");
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(!studio.undo.is_empty(), egui::Button::new(RichText::new("Undo").size(14.0))).on_hover_text("Ctrl+Z").clicked() {
            studio.undo(&mut character.paint);
        }
        if ui.add_enabled(!studio.redo.is_empty(), egui::Button::new(RichText::new("Redo").size(14.0))).on_hover_text("Ctrl+Y").clicked() {
            studio.redo(&mut character.paint);
        }
        let pieces = studio.focus.pieces();
        let what = if pieces.len() == PIECES { "Reset view".to_owned() } else { format!("Reset {}", studio.focus.name().to_lowercase()) };
        if ui.button(RichText::new(what).size(14.0)).on_hover_text("Put what you're looking at back as it was made").clicked() {
            studio.reset(&mut character.paint, &pieces);
        }
        if ui.button(RichText::new("Reset all").size(14.0)).clicked() {
            studio.reset(&mut character.paint, &(0..PIECES).collect::<Vec<_>>());
        }
    });
    ui.label(RichText::new(format!("{} voxels painted", character.paint.len())).size(12.5).color(Color32::from_gray(160)));
    if ui.button(RichText::new("Export .vox").size(14.0)).on_hover_text("Write each piece of the body as a MagicaVoxel file").clicked() {
        let dir = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir).join("Innert").join("exports").join(file_name(&character.name));
        studio.said = Some(match paint::export(&studio.models, &looks, &dir) {
            Ok(dir) => format!("Written to {}", dir.display()),
            Err(e) => format!("Couldn't write them: {e}"),
        });
    }
    if let Some(said) = &studio.said {
        ui.label(RichText::new(said).size(12.0).color(Color32::from_gray(170)));
    }
    ui.add_space(6.0);
}

/// `name` as a folder can be called
fn file_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    if s.trim().is_empty() { "character".to_owned() } else { s.trim().to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The face, seen close up, fills the picture, and what's under its middle is the head
    fn face_studio() -> (Studio, Character, Shot) {
        let character = Character::default();
        let mut studio = Studio::new(Focus::Face);
        studio.sync(&character);
        let shot = render(&studio.models, &character.looks, &studio.posed, &studio.camera.unwrap(), [300, 300]);
        (studio, character, shot)
    }

    /// The first face of the head seen from in front down column `x` of the picture, from row
    /// `from`, of material `m` if it's given
    fn find(studio: &Studio, shot: &Shot, x: f64, from: usize, m: Option<u8>) -> Hit {
        (from..300)
            .find_map(|y| shot.hit(x, y as f64).filter(|h| h.piece == HEAD && h.face == 0 && m.is_none_or(|m| paint::voxel(&studio.models[HEAD].0, h.index) == m)))
            .expect("the face is there")
    }

    /// Pictures of the painting to look at, written where INNERT_PAINT_DUMP says, from every way
    /// it can be looked at, painted and not
    #[test]
    #[ignore]
    fn pictures_to_look_at() {
        let Some(dir) = std::env::var_os("INNERT_PAINT_DUMP").map(std::path::PathBuf::from) else { return };
        let mut character = Character::default();
        for (name, paint) in [("plain", Paint::default()), ("demo", paint::demo(&character.looks))] {
            character.paint = paint;
            let mut studio = Studio::new(Focus::Face);
            studio.sync(&character);
            for focus in Focus::ALL {
                let shot = render(&studio.models, &character.looks, &studio.posed, &aim(focus, &studio.posed), [600, 500]);
                image::save_buffer(dir.join(format!("{name}_{focus:?}.png")), &shot.rgba, 600, 500, image::ColorType::Rgba8).unwrap();
            }
            println!("{name}: {} voxels painted", character.paint.len());
        }
        // The head's face as it's made, front-most voxel of each column, top row first, with the
        // middle of each row and column from the joint in centimetres
        let (m, a) = &body::models(&Looks::default())[HEAD];
        let [sx, sy, sz] = m.size.map(|v| v as i32);
        let s = paint::shift(m, a);
        for z in (0..sz).rev() {
            let mut row = format!("{:5.1} ", (z + s[2]) as f64 / BODY_RES * 100.0);
            for y in (0..sy).rev() {
                let front = (0..sx).rev().find(|&x| paint::voxel(m, [x, y, z]) != AIR);
                row += &front.map_or(" .. ".to_owned(), |x| format!("{:3}{}", paint::voxel(m, [x, y, z]), (x + s[0]).clamp(-9, 9).abs()));
            }
            println!("{row}");
        }
        println!("size {:?} shift {s:?} anchor {a:?}", m.size);
    }

    #[test]
    fn the_face_fills_the_close_up() {
        let (_, _, shot) = face_studio();
        let mut head = 0;
        for y in (0..300).step_by(10) {
            for x in (0..300).step_by(10) {
                if shot.hit(x as f64, y as f64).is_some_and(|h| h.piece == HEAD) {
                    head += 1;
                }
            }
        }
        assert!(head > 200, "{head} of 900 pixels show the head");
        let h = shot.hit(150.0, 150.0).unwrap();
        assert_eq!(h.piece, HEAD);
        assert_eq!(h.face, 0, "the face is seen from in front");
        // Its outline's round where it was hit
        let corners = shot.corners(h.piece, h.index, h.face);
        let (cx, cy) = (corners.iter().map(|c| c[0]).sum::<f64>() / 4.0, corners.iter().map(|c| c[1]).sum::<f64>() / 4.0);
        assert!((cx - 150.0).abs() < 12.0 && (cy - 150.0).abs() < 12.0, "outlined at {cx:.0}, {cy:.0}");
    }

    /// Painted with the mirror on, the voxel on the other side of the face is painted too; with
    /// it off, only the one clicked
    #[test]
    fn mirror_paints_both_sides() {
        let (mut studio, mut character, shot) = face_studio();
        let hit = find(&studio, &shot, 110.0, 140, None);
        studio.colour = Colour::Material(paint::PAINT_FIRST + 60);
        studio.click(&mut character, &hit, Tool::Paint);
        assert_eq!(character.paint.len(), 2);
        let key = paint::key_of(&studio.models[HEAD].0, &studio.models[HEAD].1, hit.index).unwrap();
        let (q, other) = paint::mirrored(&studio.base, HEAD, key).unwrap();
        assert_eq!(character.paint.get(q, other).map(|c| c.voxel), Some(paint::PAINT_FIRST + 60));
        assert_ne!(other, key);
        studio.mirror = false;
        studio.wide = true;
        let below = find(&studio, &shot, 120.0, 170, None);
        studio.click(&mut character, &below, Tool::Paint);
        let painted = character.paint.len() - 2;
        assert!((4..=9).contains(&painted), "a 3 by 3 brush painted {painted}");
        // And the picture shows it
        let after = render(&studio.models, &character.looks, &studio.posed, &studio.camera.unwrap(), [300, 300]);
        assert_ne!(after.rgba, shot.rgba);
    }

    /// Undone, a stroke's gone, redone it's back; reset takes everything off, and that's undone too
    #[test]
    fn undo_and_redo_strokes() {
        let (mut studio, mut character, shot) = face_studio();
        let hit = find(&studio, &shot, 150.0, 120, Some(crate::worldgen::material::LEATHER));
        studio.click(&mut character, &hit, Tool::Paint);
        let once = character.paint.clone();
        assert!(!once.is_empty());
        studio.colour = Colour::Look(Role::Hair);
        studio.click(&mut character, &hit, Tool::Paint);
        let twice = character.paint.clone();
        assert_ne!(once, twice);
        studio.undo(&mut character.paint);
        assert_eq!(character.paint, once);
        studio.undo(&mut character.paint);
        assert!(character.paint.is_empty());
        assert_eq!(studio.models[HEAD].0.voxels, studio.base[HEAD].0.voxels);
        studio.redo(&mut character.paint);
        studio.redo(&mut character.paint);
        assert_eq!(character.paint, twice);
        studio.reset(&mut character.paint, &[HEAD]);
        assert!(character.paint.is_empty());
        studio.undo(&mut character.paint);
        assert_eq!(character.paint, twice);
        // Painting the colour a voxel was made in leaves nothing painted there
        studio.reset(&mut character.paint, &[HEAD]);
        studio.colour = Colour::Look(Role::Skin);
        let skin = find(&studio, &shot, 150.0, 150, Some(crate::worldgen::material::LEATHER));
        assert_eq!(paint::voxel(&studio.models[HEAD].0, skin.index), crate::worldgen::material::LEATHER);
        studio.click(&mut character, &skin, Tool::Paint);
        assert!(character.paint.is_empty());
    }

    /// Added onto the face, a voxel's there in front of it, on the head only; taken away, it's gone
    #[test]
    fn voxels_added_and_taken_away() {
        let (mut studio, mut character, shot) = face_studio();
        studio.mirror = false;
        let hit = (0..300).rev().find_map(|y| shot.hit(150.0, y as f64).filter(|h| h.piece == HEAD && h.face == 0)).unwrap();
        let (m, a) = &studio.models[HEAD];
        let (under, out) = (paint::key_of(m, a, hit.index).unwrap(), paint::key_of(m, a, [hit.index[0] + 1, hit.index[1], hit.index[2]]).unwrap());
        studio.click(&mut character, &hit, Tool::Add);
        assert_eq!(character.paint.get(HEAD, out).map(|c| c.added), Some(true));
        assert_ne!(studio.voxel_at(HEAD, out).1, AIR);
        // Taken away, what shows goes first: the voxel added, then the one under it
        let again = |studio: &Studio, key| Hit { index: paint::index_of(&studio.models[HEAD].0, &studio.models[HEAD].1, key), ..hit };
        studio.click(&mut character, &again(&studio, under), Tool::Remove);
        assert_eq!(studio.voxel_at(HEAD, out).1, AIR);
        assert_eq!(character.paint.get(HEAD, out), None);
        assert_ne!(studio.voxel_at(HEAD, under).1, AIR);
        studio.click(&mut character, &again(&studio, under), Tool::Remove);
        assert_eq!(studio.voxel_at(HEAD, under).1, AIR);
        // Not on the torso
        let torso = Hit { piece: TORSO, index: [0, 0, 0], face: 0 };
        let before = character.paint.clone();
        studio.click(&mut character, &torso, Tool::Add);
        studio.click(&mut character, &torso, Tool::Remove);
        assert_eq!(character.paint, before);
    }

    /// Drawn big, close up, it's quick enough to draw again for every stroke
    #[test]
    fn drawing_is_quick() {
        let (studio, character, _) = face_studio();
        for focus in [Focus::Face, Focus::Whole] {
            let camera = aim(focus, &studio.posed);
            let start = std::time::Instant::now();
            for _ in 0..3 {
                render(&studio.models, &character.looks, &studio.posed, &camera, [900, 760]);
            }
            let each = start.elapsed().as_secs_f64() / 3.0 * 1000.0;
            println!("{focus:?}: {each:.1} ms");
            assert!(each < 60.0, "{focus:?} takes {each:.1} ms to draw");
        }
    }

    /// With the mouse as a player uses it: dragged over the face it paints a stroke, Ctrl+Z takes
    /// it off again, a right click picks up the colour under it, and Done's done
    #[test]
    fn painted_with_the_mouse() {
        use egui::{Event, PointerButton};
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut studio = Studio::new(Focus::Face);
        let mut character = Character::default();
        let screen = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(1548.0, 872.0));
        let frame = |studio: &mut Studio, character: &mut Character, events: Vec<Event>, modifiers: Modifiers| {
            let input = egui::RawInput { screen_rect: Some(screen), events, modifiers, ..Default::default() };
            let mut done = false;
            let _ = ctx.run(input, |ctx| done = window(ctx, studio, character));
            done
        };
        let none = Modifiers::NONE;
        for _ in 0..2 {
            frame(&mut studio, &mut character, vec![], none);
        }
        let rect = studio.shown.expect("the picture's shown");
        let (from, to) = (rect.center() + vec2(-60.0, 40.0), rect.center() + vec2(60.0, 40.0));
        let button = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: none };
        frame(&mut studio, &mut character, vec![Event::PointerMoved(from)], none);
        frame(&mut studio, &mut character, vec![button(from, true)], none);
        for k in 1..=6 {
            frame(&mut studio, &mut character, vec![Event::PointerMoved(from + (to - from) * k as f32 / 6.0)], none);
        }
        frame(&mut studio, &mut character, vec![button(to, false)], none);
        let stroke = character.paint.len();
        assert!(stroke >= 4, "a stroke across the face painted {stroke} voxels");
        assert_eq!(studio.undo.len(), 1, "the stroke's one thing to undo");
        let z = Event::Key { key: Key::Z, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
        frame(&mut studio, &mut character, vec![z], Modifiers::COMMAND);
        assert!(character.paint.is_empty());
        let right = |pressed| Event::PointerButton { pos: from, button: PointerButton::Secondary, pressed, modifiers: none };
        frame(&mut studio, &mut character, vec![Event::PointerMoved(from), right(true)], none);
        frame(&mut studio, &mut character, vec![right(false)], none);
        assert_eq!(studio.colour, Colour::Look(Role::Skin), "picked up off the face");
        assert!(character.paint.is_empty());
    }

    /// At the size of the window played in and shorter, the whole of it's on screen
    #[test]
    fn it_fits_a_short_window() {
        for [w, h] in [[1548.0, 872.0], [1600.0, 734.0], [1280.0, 600.0]] {
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            let mut studio = Studio::new(Focus::Face);
            let mut character = Character::default();
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h))), ..Default::default() };
            for _ in 0..3 {
                let _ = ctx.run(input.clone(), |ctx| {
                    window(ctx, &mut studio, &mut character);
                });
            }
            let shown = ctx.memory(|m| m.area_rect(egui::Id::new("character painting"))).expect("the window's drawn");
            assert!(shown.min.y >= 0.0 && shown.max.y <= h && shown.min.x >= 0.0 && shown.max.x <= w, "at {w}×{h} it's at {shown:?}");
        }
    }
}
