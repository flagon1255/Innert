//! The game's models as MagicaVoxel files, to edit in MagicaVoxel: `innert --export-models <dir>`
//! writes every model built by code as a .vox file, and at startup any file in assets\models is
//! used instead of what the code builds, see assets\models\README.md.
//!
//! Every model's voxels are material ids, see worldgen::material, and a material's id is its colour
//! index in the files' palette, so a file's colours are the game's materials, both ways: colour
//! index 54 is TEMPLE_RED. A colour that isn't one of the game's, as one changed in MagicaVoxel's
//! palette, becomes the nearest that is.
//!
//! A file only says what's where. Where the model's put in the game, its anchor or its joint, is
//! the code's, kept beside the file in a .json: the voxel the game holds still, and where the file's
//! first voxel was in MagicaVoxel's scene, so a model can grow in any direction and still sit where
//! it did.
//!
//! What's read:
//! - An animal or monster's look, drawn in baked frames: `<look>/base.vox` is its standing frame.
//!   It's used as it is for that frame, and what was changed from the code's standing frame is
//!   changed in every other frame the same way, where the same is there to change, see
//!   apply_changes(). `<look>/walk_03.vox` and the like, written by `--export-frames`, replace one
//!   frame each outright.
//! - Looks drawn alike but for their colours, as the golems are, share one file: the first of them
//!   in Look::ALL has it, and the rest are it with their materials swapped, see skin_map().
//! - A dragon's or a great serpent's pieces, a pair of wings: each piece its own file, used as it is.
//! - Every part of every family of weapons and tools, its blades, hilts, stocks and the rest, each
//!   its own file, see src\weapon_parts.rs.
//!
//! Saving a file while the game runs puts it in at once, see Reloader: worked out on another thread
//! and swapped in whole, so no frame waits for it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex, OnceLock};

use nalgebra::Vector3;
use serde::{Deserialize, Serialize};

use crate::animals::{self, Look, MODEL_RES, Model};
use crate::vox::{self, Grid};
use crate::weapons::Library;
use crate::worldgen::material::AIR;

/// Where model files are read from: assets\models beside the game's sources, like
/// assets\weapons.json, or wherever `--set models_dir=<path>` says. `--set models=off` reads none.
pub fn root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let set = setting("models_dir");
        set.map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets").join("models"))
    })
    .clone()
}

fn setting(key: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2).filter(|w| w[0] == "--set").filter_map(|w| w[1].split_once('=')).find(|(k, _)| *k == key).map(|(_, v)| v.to_string())
}

/// Set while the code's own models are wanted, as for exporting them
static CODE_ONLY: AtomicBool = AtomicBool::new(false);

/// Whether model files are read in place of the code's models
pub fn enabled() -> bool {
    static OFF: OnceLock<bool> = OnceLock::new();
    !CODE_ONLY.load(Ordering::Relaxed) && !*OFF.get_or_init(|| setting("models").is_some_and(|v| v == "off" || v == "0"))
}

// The palette

/// One of the game's materials as the files' palette has it
pub struct MaterialInfo {
    pub rgb: [u8; 3],
    pub name: String,
}

/// Every material, by id, with its colour and name as shaders\traverse.comp has them, which the
/// game's drawn with
pub fn materials() -> &'static [MaterialInfo] {
    static ALL: OnceLock<Vec<MaterialInfo>> = OnceLock::new();
    ALL.get_or_init(|| {
        let source = include_str!("../shaders/traverse.comp");
        let start = source.find("const vec3 PALETTE[").expect("traverse.comp has a palette");
        let body = &source[start..];
        let body = &body[body.find('(').map_or(0, |i| i + 1)..];
        let mut out = Vec::new();
        for line in body.lines().skip(1) {
            let line = line.trim();
            if line.starts_with(')') {
                break;
            }
            let Some(rest) = line.strip_prefix("vec3(") else { continue };
            let Some(close) = rest.find(')') else { continue };
            let v: Vec<f64> = rest[..close].split(',').filter_map(|n| n.trim().parse().ok()).collect();
            if v.len() != 3 {
                continue;
            }
            let name = line.split("//").nth(1).map_or(String::new(), |n| n.trim().to_string());
            out.push(MaterialInfo { rgb: [0, 1, 2].map(|i| (v[i].clamp(0.0, 1.0) * 255.0).round() as u8), name });
        }
        out
    })
}

/// The colour of colour indices that aren't materials: a file painted with one gets the nearest
/// material instead
const NOT_A_MATERIAL: [u8; 4] = [255, 0, 255, 255];

/// The palette the files are written with: colour index i is material i
pub fn palette() -> [[u8; 4]; 256] {
    let m = materials();
    std::array::from_fn(|i| if i == 0 { [0, 0, 0, 0] } else if i < m.len() { [m[i].rgb[0], m[i].rgb[1], m[i].rgb[2], 255] } else { NOT_A_MATERIAL })
}

/// Which material each of a file's colour indices is, see material_map()
pub fn material_map_of(file: &[[u8; 4]; 256]) -> [u8; 256] {
    material_map(file)
}

/// Which material each of a file's colour indices is: its own if its colour's still the game's,
/// otherwise the material nearest its colour
fn material_map(file: &[[u8; 4]; 256]) -> [u8; 256] {
    let ours = palette();
    let m = materials();
    std::array::from_fn(|i| {
        if i == 0 {
            return AIR;
        }
        if i < m.len() && file[i][..3] == ours[i][..3] {
            return i as u8;
        }
        let d = |k: usize| (0..3).map(|c| (file[i][c] as i32 - m[k].rgb[c] as i32).pow(2)).sum::<i32>();
        (1..m.len()).min_by_key(|&k| d(k)).unwrap_or(1) as u8
    })
}

// Models

/// A copy, as Model has no Clone
pub fn copy(m: &Model) -> Model {
    Model { size: m.size, voxels: m.voxels.clone() }
}

pub(crate) fn air(size: [u32; 3]) -> Model {
    Model { size, voxels: vec![AIR; (size[0] * size[1] * size[2]) as usize] }
}

/// A frame a look's never drawn in, which is left empty
fn is_empty(m: &Model) -> bool {
    m.size == [1, 1, 1] && m.voxels[0] == AIR
}

pub(crate) fn at(m: &Model, p: [i32; 3]) -> u8 {
    if (0..3).any(|a| p[a] < 0 || p[a] >= m.size[a] as i32) {
        return AIR;
    }
    m.voxels[(p[0] as u32 + (p[1] as u32 + p[2] as u32 * m.size[1]) * m.size[0]) as usize]
}

/// How many frames each look has, see animals::FRAMES: what comes before the jointed bosses'
/// pieces, every frame of every look. Counted once past them, as it was, it put each look's files
/// on another look's frames, every one but the first look's, see baked::written_from().
pub fn frames() -> usize {
    animals::RIG_MODELS_AT / Look::ALL.len()
}

/// Where frame `frame` of `look` is among the models, see animals::model_index()
pub fn look_model(look: Look, frame: usize) -> usize {
    Look::ALL.iter().position(|&l| l == look).unwrap() * frames() + frame
}

/// How many pieces each dragon has, see dragons::PIECES: what's left of its models once its
/// effects', FX_CRACK and the two after, are taken away
pub fn dragon_pieces() -> usize {
    (crate::dragons::MODELS - (crate::dragons::FX_CRACK + 2)) / crate::dragons::KINDS.len()
}

/// What each of a dragon's pieces is called, in the order of its models, see dragons::PIECES
const DRAGON_PIECES: [&str; 18] = [
    "torso",
    "neck_1",
    "neck_2",
    "neck_3",
    "head",
    "jaw",
    "tail_1",
    "tail_2",
    "tail_3",
    "tail_4",
    "wing_left_inner",
    "wing_left_outer",
    "wing_right_inner",
    "wing_right_outer",
    "hind_thigh",
    "hind_shin",
    "fore_thigh",
    "fore_shin",
];

pub fn dragon_piece_name(piece: usize) -> String {
    DRAGON_PIECES.get(piece).map_or_else(|| format!("piece_{piece:02}"), |n| n.to_string())
}

const SERPENT_PARTS: [&str; 3] = ["head", "jaw", "body"];

/// A name in lower case with words joined by _, for files: GraveKnight is grave_knight
pub fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// The folder a look's files are in
pub fn look_folder(look: Look) -> String {
    match look {
        // Wings spread in the hand to glide, as opposed to the kinds of wings worn, in wings\
        Look::Wings => "glider".into(),
        _ => snake(&format!("{look:?}")),
    }
}

/// What each frame of a look's file is called, see animals::POSE_WINDUP and the rest
pub fn frame_name(frame: usize) -> String {
    let walk = animals::POSE_WINDUP - 1;
    match frame {
        0 => "base".into(),
        f if f <= walk => format!("walk_{f:02}"),
        animals::POSE_WINDUP => "windup".into(),
        animals::POSE_STRIKE => "strike".into(),
        animals::POSE_SPECIAL => "special".into(),
        animals::POSE_COIL => "coil".into(),
        animals::POSE_SWEEP => "sweep".into(),
        animals::POSE_RAISE => "raise".into(),
        animals::POSE_SPIKES => "spikes".into(),
        f => format!("between_{:02}", f - animals::POSE_SPIKES),
    }
}

/// Voxels a meter of a look's model, see Look::resolution()
pub fn look_res(look: Look) -> f64 {
    match look {
        Look::GrappleHook => 3.0 * MODEL_RES,
        Look::Wings => 4.0 * MODEL_RES,
        _ if animals::sealife::is_small(look) => animals::sealife::FINE * MODEL_RES,
        _ => MODEL_RES,
    }
}

/// Voxels a meter of the wings' models, see wings::RES
pub const WING_RES: f64 = 6.0 * MODEL_RES;

/// What's kept beside each file: where its model goes in the game. `pivot` is the voxel the game
/// holds still, its joint or the middle of its feet, in the file's voxels as written; `origin` is
/// where the file's first voxel was put in MagicaVoxel's scene, the pivot at the scene's middle.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Sidecar {
    pub what: String,
    pub voxels_per_meter: f64,
    pub size: [u32; 3],
    pub pivot: [f64; 3],
    /// For a model placed by its anchor, the middle of its bottom, where that is from its joint
    /// in meters, as the code made it
    #[serde(default)]
    pub anchor: Option<[f64; 3]>,
    pub origin: [i32; 3],
    /// For a weapon's part, where on the weapon it joins the rest and where it's held, its tip
    /// and the like, in the weapon's own meters, as the code has them: kept by the code whatever
    /// the file's shape, see src\weapon_parts.rs
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub points: BTreeMap<String, [f64; 3]>,
    #[serde(default)]
    pub note: String,
    /// How long the file was as it was written and a hash of it: a file still the same was never
    /// edited, so the code's model is used, as it may have changed since, see unchanged()
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written: Option<(u64, String)>,
}

impl Sidecar {
    fn new(what: String, res: f64, size: [u32; 3], anchor: Option<Vector3<f64>>, note: &str) -> Self {
        let pivot = pivot_of(size, anchor, res);
        Sidecar { what, voxels_per_meter: res, size, pivot, anchor: anchor.map(|a| [a.x, a.y, a.z]), origin: origin_of(pivot), points: BTreeMap::new(), note: note.into(), written: None }
    }
}

/// The voxel the game holds still in a model `size` big: the middle of its bottom for a look's
/// frame, which is where the animal stands; for a model with an anchor, the joint it was made round
pub(crate) fn pivot_of(size: [u32; 3], anchor: Option<Vector3<f64>>, res: f64) -> [f64; 3] {
    let a = anchor.unwrap_or_else(Vector3::zeros);
    [size[0] as f64 * 0.5 - a.x * res, size[1] as f64 * 0.5 - a.y * res, -a.z * res]
}

/// Where a file's first voxel goes in MagicaVoxel's scene, so its pivot's at the scene's middle
pub(crate) fn origin_of(pivot: [f64; 3]) -> [i32; 3] {
    pivot.map(|p| -(p.round() as i32))
}

/// A file's voxels in its model's own grid, as the code made it: a box of materials whose first
/// voxel is at `lo` there, which may be outside it
pub(crate) struct Edit {
    pub lo: [i32; 3],
    pub size: [u32; 3],
    pub voxels: Vec<u8>,
}

impl Edit {
    /// Its solid voxels, where they are in the model's grid
    pub(crate) fn solid(&self) -> Vec<([i32; 3], u8)> {
        let mut out = Vec::new();
        let [sx, sy, sz] = self.size;
        for z in 0..sz {
            for y in 0..sy {
                for x in 0..sx {
                    let v = self.voxels[(x + (y + z * sy) * sx) as usize];
                    if v != AIR {
                        out.push(([self.lo[0] + x as i32, self.lo[1] + y as i32, self.lo[2] + z as i32], v));
                    }
                }
            }
        }
        out
    }
}

/// Reads `path` and the .json beside it as an edit of a model `size` big, held still at `pivot`
pub(crate) fn read_edit(path: &Path, pivot: [f64; 3]) -> Result<Edit, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let file = vox::read(&bytes)?;
    let sidecar: Option<Sidecar> = std::fs::read_to_string(path.with_extension("json")).ok().and_then(|t| serde_json::from_str(&t).ok());
    let origin = sidecar.map_or_else(|| origin_of(pivot), |s| s.origin);
    let map = material_map(&file.palette);
    // Placed in the scene, a file's voxels are where it put them; a file with no scene, as other
    // programs write, is taken to be where it was written
    let first = if file.placed { file.grid.origin } else { origin };
    Ok(Edit { lo: [0, 1, 2].map(|a| first[a] - origin[a]), size: file.grid.size, voxels: file.grid.voxels.iter().map(|&i| map[i as usize]).collect() })
}

/// How a model can grow to take in what's put in it
#[derive(Clone, Copy)]
pub(crate) enum Grow {
    /// Held by the middle of its bottom, which stays where it is: wider the same either side,
    /// taller only upwards, nothing below its bottom, and no more than `cap` a side
    Fixed { cap: u32 },
    /// Held by an anchor, which moves with it, any way, no more than 1023 a side
    Free,
}

/// `base` with `edits` put in it, each a material at a voxel of its grid, growing as `grow` lets
/// it to take in those outside it. Returns the model, and where its first voxel was in base's grid.
pub(crate) fn place(base: &Model, edits: &[([i32; 3], u8)], grow: Grow) -> (Model, [i32; 3]) {
    let s = base.size.map(|v| v as i32);
    let (mut lo, mut hi) = ([0i32; 3], s);
    for &(p, m) in edits {
        if m != AIR {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a] + 1);
            }
        }
    }
    match grow {
        Grow::Fixed { cap } => {
            let cap = cap as i32;
            for a in 0..2 {
                let k = (-lo[a]).max(hi[a] - s[a]).max(0).min(((cap - s[a]) / 2).max(0));
                lo[a] = -k;
                hi[a] = s[a] + k;
            }
            lo[2] = 0;
            hi[2] = hi[2].min(cap.max(s[2]));
        }
        Grow::Free => {
            for a in 0..3 {
                // Kept round the code's box, cut off on the far side past what the shader can hold
                if hi[a] - lo[a] > 1023 {
                    lo[a] = lo[a].max(s[a] - 1023).min(0);
                    hi[a] = lo[a] + 1023;
                }
            }
        }
    }
    let size = [0, 1, 2].map(|a| (hi[a] - lo[a]).max(1) as u32);
    let mut out = air(size);
    let set = |out: &mut Model, p: [i32; 3], v: u8| {
        let q = [p[0] - lo[0], p[1] - lo[1], p[2] - lo[2]];
        if (0..3).all(|a| q[a] >= 0 && q[a] < size[a] as i32) {
            out.voxels[(q[0] as u32 + (q[1] as u32 + q[2] as u32 * size[1]) * size[0]) as usize] = v;
        }
    };
    for z in 0..s[2] {
        for y in 0..s[1] {
            for x in 0..s[0] {
                let v = base.voxels[(x + (y + z * s[1]) * s[0]) as usize];
                if v != AIR {
                    set(&mut out, [x, y, z], v);
                }
            }
        }
    }
    for &(p, m) in edits {
        set(&mut out, p, m);
    }
    (out, lo)
}

/// Where the anchor of a model `size` big with its anchor at `anchor` goes once it's the box from
/// `lo` to `size2` from there in its grid, at `res` voxels a meter
pub(crate) fn moved_anchor(anchor: Vector3<f64>, size: [u32; 3], lo: [i32; 3], size2: [u32; 3], res: f64) -> Vector3<f64> {
    let mid = |a: usize| (2 * lo[a] + size2[a] as i32 - size[a] as i32) as f64 * 0.5 / res;
    anchor + Vector3::new(mid(0), mid(1), lo[2] as f64 / res)
}

/// A file read whole into the place of a model `size` big held as `grow` says
pub(crate) fn conform(edit: &Edit, size: [u32; 3], grow: Grow) -> (Model, [i32; 3]) {
    place(&air(size), &edit.solid(), grow)
}

/// What a file changed from the code's model: each voxel that's different, where it is in the
/// code model's grid, what it is now and what it was
type Changes = Vec<([i32; 3], u8, u8)>;

/// What `edited`, whose first voxel's at `lo` in `code`'s grid, has different from `code`
pub(crate) fn changes(code: &Model, edited: &Model, lo: [i32; 3]) -> Changes {
    let mut out = Vec::new();
    let [sx, sy, sz] = edited.size;
    for z in 0..sz {
        for y in 0..sy {
            for x in 0..sx {
                let p = [x as i32 + lo[0], y as i32 + lo[1], z as i32 + lo[2]];
                let (now, was) = (edited.voxels[(x + (y + z * sy) * sx) as usize], at(code, p));
                if now != was {
                    out.push((p, now, was));
                }
            }
        }
    }
    // And what's in the code's model outside the edited one's box, which is gone
    let [cx, cy, cz] = code.size;
    for z in 0..cz as i32 {
        for y in 0..cy as i32 {
            for x in 0..cx as i32 {
                let q = [x - lo[0], y - lo[1], z - lo[2]];
                let inside = (0..3).all(|a| q[a] >= 0 && q[a] < edited.size[a] as i32);
                let was = at(code, [x, y, z]);
                if !inside && was != AIR {
                    out.push(([x, y, z], AIR, was));
                }
            }
        }
    }
    out
}

/// `changes` made to another model like the one they were made to, `offset` voxels from it: a
/// voxel taken away or turned another material only where the same material's there in it, one
/// added only where it has air. So what moves between frames keeps its own, and what stays put
/// takes the change.
fn apply_changes(model: &Model, changes: &Changes, offset: [i32; 3], grow: Grow) -> (Model, [i32; 3]) {
    let mut edits = Vec::new();
    for &(p, now, was) in changes {
        let q = [p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]];
        let here = at(model, q);
        let ok = if was == AIR { here == AIR } else { here == was };
        if ok {
            edits.push((q, now));
        }
    }
    place(model, &edits, grow)
}

/// The material each of `a`'s is in `b`, if `b` is `a` with only its materials changed, a material
/// always the same other one, and air left air: the same shape in other colours. Every model in
/// each has to be the same size.
pub fn skin_map(a: &[&Model], b: &[&Model]) -> Option<[u8; 256]> {
    if a.len() != b.len() || a.iter().zip(b).any(|(x, y)| x.size != y.size) {
        return None;
    }
    let mut map = [None::<u8>; 256];
    map[AIR as usize] = Some(AIR);
    for (x, y) in a.iter().zip(b) {
        for (&u, &v) in x.voxels.iter().zip(&y.voxels) {
            match map[u as usize] {
                Some(w) if w != v => return None,
                Some(_) => {}
                None => map[u as usize] = Some(v),
            }
        }
    }
    // Something made solid in one has to be solid in the other
    if map.iter().enumerate().any(|(u, m)| u != AIR as usize && *m == Some(AIR)) {
        return None;
    }
    Some(std::array::from_fn(|i| map[i].unwrap_or(i as u8)))
}

fn recolor(m: &Model, map: &[u8; 256]) -> Model {
    Model { size: m.size, voxels: m.voxels.iter().map(|&v| map[v as usize]).collect() }
}

/// Every file under `dir`, by its path from there without .vox, with / between folders
pub(crate) fn scan(dir: &Path) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            // From the listing itself, rather than asking after each file again
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vox"))
                && let Ok(rel) = p.with_extension("").strip_prefix(dir)
            {
                out.insert(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out
}

fn hash_of(bytes: &[u8]) -> String {
    // Eight bytes at a time, as a folder full of files is checked at startup
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ bytes.len() as u64;
    for chunk in bytes.chunks(8) {
        let mut w = [0u8; 8];
        w[..chunk.len()].copy_from_slice(chunk);
        h = (h ^ u64::from_le_bytes(w)).wrapping_mul(0x0100_0000_01b3);
        h ^= h >> 29;
    }
    format!("{h:016x}")
}

/// How long `model`'s file is, written as a look's frame at `res` voxels a meter, and its hash,
/// as its .json keeps them: for telling which model a file was written from, see
/// baked::legacy()
pub(crate) fn written_as(model: &Model, res: f64) -> (u64, String) {
    written_with(model, res, &palette())
}

/// The same, as written with the palette as it was before the building materials were added, see
/// src\materials.rs: its colours past clay bank not materials. A file written then says so.
pub(crate) fn written_as_before(model: &Model, res: f64) -> (u64, String) {
    let mut old = palette();
    for colour in old.iter_mut().skip(crate::worldgen::material::CLAY_BANK as usize + 1) {
        *colour = NOT_A_MATERIAL;
    }
    written_with(model, res, &old)
}

fn written_with(model: &Model, res: f64, palette: &[[u8; 4]; 256]) -> (u64, String) {
    let grid = Grid { size: model.size, voxels: model.voxels.clone(), origin: origin_of(pivot_of(model.size, None, res)) };
    let bytes = vox::write(&grid, palette);
    (bytes.len() as u64, hash_of(&bytes))
}

/// Whether the file at `path` is just as it was exported, so never edited: then the code's model
/// is drawn, as it was, or as it's become since
pub(crate) fn unchanged(path: &Path) -> bool {
    let Some((len, hash)) = std::fs::read_to_string(path.with_extension("json")).ok().and_then(|t| serde_json::from_str::<Sidecar>(&t).ok()).and_then(|s| s.written) else {
        return false;
    };
    std::fs::metadata(path).is_ok_and(|m| m.len() == len) && std::fs::read(path).is_ok_and(|b| hash_of(&b) == hash)
}

/// The files under `dir` that have been edited since they were exported, see scan() and
/// unchanged(); the weapons' parts' all, as they're looked at only as they're needed
pub(crate) fn edited(dir: &Path) -> HashSet<String> {
    let mut known = EDITED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((d, files)) = known.as_ref()
        && d == dir
    {
        return files.clone();
    }
    // Looked over on every core at once, as there are a good many, the weapons' parts aside
    let (weapons, rest): (Vec<String>, Vec<String>) = scan(dir).into_iter().partition(|f| f.starts_with("weapons/"));
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8);
    let chunk = rest.len().div_ceil(workers).max(1);
    let changed: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = rest.chunks(chunk).map(|names| scope.spawn(move || names.iter().filter(|f| !unchanged(&file_of(dir, f))).cloned().collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    let files: HashSet<String> = weapons.into_iter().chain(changed).collect();
    *known = Some((dir.to_path_buf(), files.clone()));
    files
}

/// The files edited, as edited() last found them, till they change
static EDITED: Mutex<Option<(PathBuf, HashSet<String>)>> = Mutex::new(None);

pub fn file_of(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.vox"))
}

/// What's said of what was read, kept for the editor to show, newest last
pub fn log(line: String) {
    eprintln!("{line}");
    let mut log = lines();
    log.push(line);
    if log.len() > 40 {
        log.remove(0);
    }
}

pub fn recent_log() -> Vec<String> {
    lines().clone()
}

/// What's been said, apart from State so it can be said while that's held
fn lines() -> std::sync::MutexGuard<'static, Vec<String>> {
    static LINES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    LINES.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// What's kept between reading files and reading them again
#[derive(Default)]
struct State {
    /// The code's own models replaced by files, by their index among Game's models, for working
    /// out the files' changes from again
    code: HashMap<usize, Model>,
    /// The code's wings, and whether they've been replaced
    code_wings: Option<Vec<(Model, Vector3<f64>)>>,
    /// The code's effects and wards, once any are replaced
    code_effects: Option<Vec<(Model, Vector3<f64>)>>,
    code_wards: Option<Vec<(Model, Vector3<f64>)>>,
}

fn state() -> std::sync::MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// The frames of a look as its files have them: `code` the code's, every frame, empty ones too
fn look_frames(dir: &Path, files: &HashSet<String>, look: Look, code: &[&Model]) -> Vec<Model> {
    let folder = look_folder(look);
    let res = look_res(look);
    let cap = Grow::Fixed { cap: 255 };
    let mut out: Vec<Model> = code.iter().map(|m| copy(m)).collect();
    let base = format!("{folder}/base");
    if files.contains(&base) {
        match read_edit(&file_of(dir, &base), pivot_of(code[0].size, None, res)) {
            Ok(edit) => {
                let (edited, lo) = conform(&edit, code[0].size, cap);
                let changed = changes(code[0], &edited, lo);
                if !changed.is_empty() {
                    for f in 1..code.len() {
                        if is_empty(code[f]) {
                            continue;
                        }
                        // Frames are each as big as they need, their middles over the same spot
                        let offset = [0, 1].map(|a| (code[f].size[a] as i32 - code[0].size[a] as i32).div_euclid(2));
                        out[f] = apply_changes(code[f], &changed, [offset[0], offset[1], 0], cap).0;
                    }
                }
                out[0] = edited;
            }
            Err(e) => log(format!("{base}.vox: {e}")),
        }
    }
    for (f, frame) in code.iter().enumerate().skip(1) {
        let name = format!("{folder}/{}", frame_name(f));
        if !is_empty(frame) && files.contains(&name) {
            match read_edit(&file_of(dir, &name), pivot_of(frame.size, None, res)) {
                Ok(edit) => out[f] = conform(&edit, frame.size, cap).0,
                Err(e) => log(format!("{name}.vox: {e}")),
            }
        }
    }
    out
}

/// Whether a look has files of its own
fn has_files(files: &HashSet<String>, look: Look) -> bool {
    let folder = look_folder(look);
    (0..frames()).any(|f| files.contains(&format!("{folder}/{}", frame_name(f)))) || !crate::baked::files_of(look, files).is_empty()
}

/// Where piece `p` of look `look`, baked from pieces, is among the models, see
/// animals::BAKED_MODELS_AT
pub fn baked_model(look: Look, p: usize) -> Option<usize> {
    Some(animals::BAKED_MODELS_AT + crate::baked::kind(look)? * crate::baked::PIECES + p)
}

/// What's said in a baked look's pieces' .json
pub const PIECE_NOTE: &str = "A piece its frames are baked from: the pivot is its joint. What you change here goes with it in every \
    frame it's drawn in, walking and attacking.";

/// A look whose frames are baked from pieces, see src\baked.rs, with its files read in: its pieces'
/// files, an old standing frame's, its animation files, and files replacing frames outright; or, if
/// it has none of its own and is another look's shape in its own colours, that one's pieces'
/// files, recoloured. Its frames, every one, and its pieces' models, PIECES of them; None if
/// nothing of it's changed.
fn baked_look(dir: &Path, files: &HashSet<String>, look: Look) -> Option<(Vec<Model>, Vec<Model>)> {
    let rig = crate::baked::rig(look)?;
    let own = has_files(files, look);
    let source = crate::baked::skin_of(look).filter(|&(from, _)| !own && !crate::baked::files_of(from, files).is_empty());
    if !own && source.is_none() && !crate::baked::has_anims(look) {
        return None;
    }
    let pieces: Vec<crate::baked::Piece> = match source {
        Some((from, map)) => crate::baked::read_pieces(from, dir, files, &look_folder(from)).iter().map(|p| p.recolored(&map)).collect(),
        None => crate::baked::read_pieces(look, dir, files, &look_folder(look)),
    };
    crate::baked::laid_out(look, &pieces);
    let mut frames = rig.bake(&pieces, None);
    // Files replacing frames outright
    let folder = look_folder(look);
    for (f, frame) in frames.iter_mut().enumerate().skip(1) {
        let name = format!("{folder}/{}", frame_name(f));
        if !is_empty(frame) && files.contains(&name) {
            match read_edit(&file_of(dir, &name), pivot_of(frame.size, None, MODEL_RES)) {
                Ok(edit) => *frame = conform(&edit, frame.size, Grow::Fixed { cap: 255 }).0,
                Err(e) => log(format!("{name}.vox: {e}")),
            }
        }
    }
    let models = (0..crate::baked::PIECES).map(|p| pieces.get(p).map_or_else(|| air([1, 1, 1]), |piece| piece.model.clone())).collect();
    Some((frames, models))
}

/// The look whose file `look` is drawn from in other colours, if it shares one, with the
/// materials swapped: the first in Look::ALL that's the same shape, see skin_map(). `frames_of`
/// gives the code's frames of a look. Looks baked from pieces share their pieces' files instead,
/// see baked::skin_of().
pub fn skin_of<'a>(look: Look, frames_of: &dyn Fn(Look) -> Vec<&'a Model>) -> Option<(Look, [u8; 256])> {
    if crate::baked::baked(look) {
        return None;
    }
    let mine = frames_of(look);
    if mine.iter().all(|m| is_empty(m)) {
        return None;
    }
    for &other in Look::ALL.iter().take_while(|&&l| l != look) {
        if other.build() != look.build() || crate::baked::baked(other) {
            continue;
        }
        let theirs = frames_of(other);
        if theirs[0].size != mine[0].size {
            continue;
        }
        if let Some(map) = skin_map(&theirs, &mine) {
            // Only from a look that's drawn from its own, not from one drawn from another's
            if skin_of(other, frames_of).is_none() {
                return Some((other, map));
            }
        }
    }
    None
}

/// The looks' frames, the first models of animals::animal_models(), with their files read in.
/// Called as the game's models are made, see Game::new().
pub fn apply_looks(mut models: Vec<Model>) -> Vec<Model> {
    if !enabled() {
        return models;
    }
    let dir = root();
    let files = edited(&dir);
    if files.is_empty() && !crate::baked::KINDS.iter().any(|&l| crate::baked::has_anims(l)) {
        return models;
    }
    let start = std::time::Instant::now();
    let n = frames();
    let mut done: BTreeMap<usize, Vec<Model>> = BTreeMap::new();
    let index = |l: Look| Look::ALL.iter().position(|&k| k == l).unwrap();
    // Looks baked from pieces, their pieces' models too
    let mut s = state();
    for &look in &crate::baked::KINDS {
        if let Some((frames, pieces)) = baked_look(&dir, &files, look) {
            done.insert(index(look), frames);
            for (p, m) in pieces.into_iter().enumerate() {
                let i = baked_model(look, p).unwrap();
                let old = std::mem::replace(&mut models[i], m);
                s.code.insert(i, old);
            }
        }
    }
    drop(s);
    for &look in &Look::ALL {
        if !crate::baked::baked(look) && has_files(&files, look) {
            let k = look_model(look, 0);
            let code: Vec<&Model> = models[k..k + n].iter().collect();
            done.insert(index(look), look_frames(&dir, &files, look, &code));
        }
    }
    // Looks drawn from another's file in their own colours
    let mut skins = Vec::new();
    if !done.is_empty() {
        let frames_of = |l: Look| -> Vec<&Model> {
            let k = look_model(l, 0);
            models[k..k + n].iter().collect()
        };
        for &look in &Look::ALL {
            if done.contains_key(&index(look)) || crate::baked::baked(look) {
                continue;
            }
            if let Some((from, map)) = skin_of(look, &frames_of)
                && let Some(frames) = done.get(&index(from))
            {
                skins.push((index(look), frames.iter().map(|m| recolor(m, &map)).collect::<Vec<_>>()));
            }
        }
    }
    let changed = done.len() + skins.len();
    let mut s = state();
    for (look, frames) in done.into_iter().chain(skins) {
        let k = look_model(Look::ALL[look], 0);
        for (f, m) in frames.into_iter().enumerate() {
            let old = std::mem::replace(&mut models[k + f], m);
            s.code.insert(k + f, old);
        }
    }
    drop(s);
    log(format!("Model files: {changed} looks read from {} in {:.0} ms", dir.display(), start.elapsed().as_secs_f64() * 1000.0));
    models
}

/// Models placed by an anchor, each with its file if there is one: `name(i)` gives the file of
/// model i, if it has one. Used for dragons' and great serpents' pieces and wings, which are each
/// read whole.
fn apply_anchored(made: &mut [(Model, Vector3<f64>)], res: f64, name: &dyn Fn(usize) -> Option<String>) -> usize {
    if !enabled() {
        return 0;
    }
    let dir = root();
    let files = edited(&dir);
    let mut changed = 0;
    for (i, (model, anchor)) in made.iter_mut().enumerate() {
        let Some(name) = name(i).filter(|n| files.contains(n)) else { continue };
        if is_empty(model) {
            continue;
        }
        match read_edit(&file_of(&dir, &name), pivot_of(model.size, Some(*anchor), res)) {
            Ok(edit) => {
                let (m, lo) = conform(&edit, model.size, Grow::Free);
                *anchor = moved_anchor(*anchor, model.size, lo, m.size, res);
                *model = m;
                changed += 1;
            }
            Err(e) => log(format!("{name}.vox: {e}")),
        }
    }
    changed
}

fn dragon_file(i: usize) -> Option<String> {
    let pieces = dragon_pieces();
    let kind = *crate::dragons::KINDS.get(i / pieces)?;
    Some(format!("dragon/{}/{}", snake(&format!("{kind:?}")), dragon_piece_name(i % pieces)))
}

pub fn boss_file(i: usize) -> Option<String> {
    let look = *crate::rig::KINDS.get(i / crate::rig::PIECES)?;
    Some(format!("boss/{}/{}", snake(&format!("{look:?}")), crate::rig::edit::piece_name(look, i % crate::rig::PIECES)))
}

/// A kraken's or a squid's part, in boss\, see bosses::kraken::PART_NAMES
fn kraken_file(i: usize) -> Option<String> {
    use crate::bosses::kraken::{LOOKS, PART_NAMES};
    let look = *LOOKS.get(i / PART_NAMES.len())?;
    Some(format!("boss/{}/{}", snake(&format!("{look:?}")), PART_NAMES[i % PART_NAMES.len()]))
}

fn serpent_file(i: usize) -> Option<String> {
    let g = crate::serpents::GREATS.get(i / 3)?;
    Some(format!("serpent/{}/{}", snake(&format!("{:?}", g.look)), SERPENT_PARTS[i % 3]))
}

fn wing_file(i: usize) -> Option<String> {
    let kinds = crate::wings::KINDS.len();
    Some(if i < kinds { format!("wings/{}", crate::wings::KINDS[i].id) } else { format!("wings/trail_{}", i - kinds + 1) })
}

/// What the dragons breathe and leave burning, after their pieces, see dragons::FX_PUFF
fn dragon_fx_file(fx: usize) -> Option<String> {
    let element = ["fire", "frost", "storm", "poison"];
    Some(match fx {
        _ if fx < crate::dragons::FX_FLAME => format!("dragon/fx/breath_{}_{}", element[fx / 2 % 4], fx % 2 + 1),
        _ if fx < crate::dragons::FX_CRACK => format!("dragon/fx/flame_{}_{}", element[(fx - crate::dragons::FX_FLAME) / 2 % 4], fx % 2 + 1),
        _ => format!("dragon/fx/crack_{}", fx - crate::dragons::FX_CRACK + 1),
    })
}

/// What avians and gargoyles throw, call down and break into, after their pieces, see
/// avians::FX_FEATHER
fn avian_fx_file(fx: usize) -> Option<String> {
    Some(match fx {
        _ if fx < crate::avians::FX_BOLT => format!("avian/fx/feather_{}", fx + 1),
        _ if fx < crate::avians::FX_FORK => "avian/fx/bolt".to_string(),
        _ if fx < crate::avians::FX_SHARD => "avian/fx/fork".to_string(),
        _ => format!("avian/fx/shard_{}", fx - crate::avians::FX_SHARD + 1),
    })
}

/// The arrows, bolts, shot, spells, sparks, smoke, pools and the player's arms, in the order
/// weapons::effect_models() makes them
/// Where the special ammo's models start among the effects, see weapons::AMMO_STYLES: last among
/// them, the first of as many as there are effects told here
static AMMO_AT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(usize::MAX);

fn count_effects(n: usize) {
    AMMO_AT.store(n.saturating_sub(crate::weapons::AMMO_STYLES.len()), Ordering::Relaxed);
}

fn effect_file(i: usize) -> Option<String> {
    // Each kind of special ammo after them, by its id, see weapons::AMMO_STYLES
    let ammo = AMMO_AT.load(Ordering::Relaxed);
    Some(match crate::weapons::AMMO_STYLES.get(i.wrapping_sub(ammo)) {
        Some(style) if i >= ammo => format!("ammo/{}", style.id),
        _ => format!("effects/{i:03}"),
    })
}

/// The wards round raid bosses, in the order guardians::models() makes them
fn ward_file(i: usize) -> Option<String> {
    Some(format!("wards/{i:02}"))
}

/// Models whose anchors stay as the code has them, each read whole from its file if it's been
/// edited, growing only round its anchor: effects, wards, what dragons breathe. Their voxels a
/// meter aren't known here, so their files are placed as if 16.
fn apply_fixed(made: &mut [(Model, Vector3<f64>)], name: &dyn Fn(usize) -> Option<String>) -> usize {
    if !enabled() {
        return 0;
    }
    let dir = root();
    let files = edited(&dir);
    let mut changed = 0;
    for (i, (model, anchor)) in made.iter_mut().enumerate() {
        let Some(name) = name(i).filter(|n| files.contains(n)) else { continue };
        match read_edit(&file_of(&dir, &name), pivot_of(model.size, Some(*anchor), MODEL_RES)) {
            Ok(edit) => {
                *model = conform(&edit, model.size, Grow::Fixed { cap: 1023 }).0;
                changed += 1;
            }
            Err(e) => log(format!("{name}.vox: {e}")),
        }
    }
    changed
}

/// The arrows, spells, sparks and the player's arms and the rest, with the files read in, see
/// Combat::new()
pub fn effects(mut made: Vec<(Model, Vector3<f64>)>) -> Vec<(Model, Vector3<f64>)> {
    count_effects(made.len());
    if enabled() && edited(&root()).iter().any(|f| f.starts_with("effects/") || f.starts_with("ammo/")) {
        let code: Vec<(Model, Vector3<f64>)> = made.iter().map(|(m, a)| (copy(m), *a)).collect();
        count_effects(made.len());
        let changed = apply_fixed(&mut made, &effect_file);
        if changed > 0 {
            state().code_effects = Some(code);
            log(format!("Model files: {changed} effects read"));
        }
    }
    made
}

/// The wards round raid bosses, with the files read in, see guardians::models()
pub fn wards(mut made: Vec<(Model, Vector3<f64>)>) -> Vec<(Model, Vector3<f64>)> {
    if enabled() && edited(&root()).iter().any(|f| f.starts_with("wards/")) {
        let code: Vec<(Model, Vector3<f64>)> = made.iter().map(|(m, a)| (copy(m), *a)).collect();
        if apply_fixed(&mut made, &ward_file) > 0 {
            state().code_wards = Some(code);
        }
    }
    made
}

/// Every dragon's pieces, and after them their effects, with the files read in, see
/// dragons::build_models(). Dragons drawn alike but for their colours share their files, see
/// skin_map().
pub fn dragons(made: &mut [(Model, Vector3<f64>)]) {
    let kinds = crate::dragons::KINDS.len();
    jointed(made, kinds, dragon_pieces(), "dragon/", &dragon_file, "dragon pieces");
    apply_fixed(&mut made[kinds * dragon_pieces()..], &dragon_fx_file);
}

/// Every jointed boss's pieces, with the files read in, see rig::build_models(). Bosses drawn
/// alike but for their colours, as the golems are, share their files.
pub fn bosses(made: &mut [(Model, Vector3<f64>)]) {
    jointed(made, crate::rig::KINDS.len(), crate::rig::PIECES, "boss/", &boss_file, "boss pieces");
}

/// Every avian's and gargoyle's pieces, and after them what they throw, with the files read in,
/// see avians::build_models()
pub fn avians(made: &mut [(Model, Vector3<f64>)]) {
    let kinds = crate::avians::KINDS.len();
    jointed(made, kinds, avian_pieces(), "avian/", &avian_file, "avian and gargoyle pieces");
    apply_fixed(&mut made[kinds * avian_pieces()..], &avian_fx_file);
}

/// How many pieces each avian and gargoyle has, see avians::PIECES
pub fn avian_pieces() -> usize {
    crate::avians::PIECE_NAMES.len()
}

pub fn avian_file(i: usize) -> Option<String> {
    let look = *crate::avians::KINDS.get(i / avian_pieces())?;
    Some(format!("avian/{}/{}", snake(&format!("{look:?}")), crate::avians::PIECE_NAMES[i % avian_pieces()]))
}

/// The pieces of `kinds` kinds of jointed things, `pieces` each, with the files read in: `file`
/// gives a piece's file, all under `prefix`. A kind with no files of its own that's another drawn
/// in other colours is drawn from that one's files, recoloured, see skin_map().
fn jointed(made: &mut [(Model, Vector3<f64>)], kinds: usize, pieces: usize, prefix: &str, file: &dyn Fn(usize) -> Option<String>, what: &str) {
    if !enabled() || !edited(&root()).iter().any(|f| f.starts_with(prefix)) {
        return;
    }
    // Which kind each shares its files with, worked out on the code's models
    let shares: Vec<Option<(usize, [u8; 256])>> = (0..kinds)
        .map(|k| {
            let mine: Vec<&Model> = made[k * pieces..(k + 1) * pieces].iter().map(|(m, _)| m).collect();
            (0..k).find_map(|j| {
                let theirs: Vec<&Model> = made[j * pieces..(j + 1) * pieces].iter().map(|(m, _)| m).collect();
                skin_map(&theirs, &mine).map(|map| (j, map))
            })
        })
        .collect();
    let files = edited(&root());
    let owns = |k: usize| (0..pieces).any(|p| file(k * pieces + p).is_some_and(|f| files.contains(&f)));
    let changed = apply_anchored(&mut made[..kinds * pieces], MODEL_RES, &|i| file(i).filter(|_| shares[i / pieces].is_none_or(|_| owns(i / pieces))));
    for (k, share) in shares.iter().enumerate() {
        if let Some((j, map)) = share
            && !owns(k)
            && owns(*j)
        {
            for p in 0..pieces {
                let (m, a) = (recolor(&made[j * pieces + p].0, map), made[j * pieces + p].1);
                made[k * pieces + p] = (m, a);
            }
        }
    }
    if changed > 0 {
        log(format!("Model files: {changed} {what} read"));
    }
}

/// The great serpents' heads, jaws and pieces of body, with the files read in, see
/// serpents::parts_made()
pub fn serpents(made: &mut [(Model, Vector3<f64>)]) {
    let changed = apply_anchored(made, MODEL_RES, &serpent_file);
    if changed > 0 {
        log(format!("Model files: {changed} great serpent pieces read"));
    }
}

/// The krakens' mantles, heads and pieces of arm, with the files read in, see
/// bosses::kraken::parts_made()
pub fn krakens(made: &mut [(Model, Vector3<f64>)]) {
    let changed = apply_anchored(made, MODEL_RES, &kraken_file);
    if changed > 0 {
        log(format!("Model files: {changed} kraken pieces read"));
    }
}

/// Every kind of wings, then the motes of their trails, with the files read in, see wings::models()
pub fn wings(mut made: Vec<(Model, Vector3<f64>)>) -> Vec<(Model, Vector3<f64>)> {
    if enabled() && edited(&root()).iter().any(|f| f.starts_with("wings/")) {
        let code: Vec<(Model, Vector3<f64>)> = made.iter().map(|(m, a)| (copy(m), *a)).collect();
        let changed = apply_anchored(&mut made, WING_RES, &wing_file);
        if changed > 0 {
            state().code_wings = Some(code);
            log(format!("Model files: {changed} wings read"));
        }
    }
    made
}

/// The player's body's pieces, see character::body, with the files read in: a piece edited in
/// assets\models\player\ is drawn as it's been edited whatever the player looks like, its leather
/// still in the colour of their skin, gloves or boots, and its cloth in that of their hair or clothes
pub fn player(mut made: Vec<(Model, Vector3<f64>)>, _looks: &crate::character::Looks) -> Vec<(Model, Vector3<f64>)> {
    if enabled() && edited(&root()).iter().any(|f| f.starts_with("player/")) {
        let changed = apply_anchored(&mut made, crate::character::body::BODY_RES, &player_file);
        if changed > 0 {
            log(format!("Model files: {changed} pieces of the player's body read"));
        }
    }
    made
}

fn player_file(i: usize) -> Option<String> {
    Some(format!("player/{}", crate::character::body::PIECE_NAMES.get(i)?))
}

/// What's said in the files of the player's body
const PLAYER_NOTE: &str = "A piece of the player's own body, seen only in its shadow and reflections, never by the eye: the pivot is its joint. \
    Its leather (73) is drawn in the colour the player chose for its skin, gloves, belt or boots, and its cloth (74) in that of their hair or clothes; \
    everything else in its own colour. Drawn as edited for every look; as exported it's a new character's.";

// Writing the files

/// Writes `model` to `name`.vox under `dir`, with its .json beside it
pub fn write_model(dir: &Path, name: &str, model: &Model, res: f64, anchor: Option<Vector3<f64>>, what: &str, note: &str) -> Result<PathBuf, String> {
    write_sidecar(dir, name, model, Sidecar::new(what.into(), res, model.size, anchor, note))
}

/// Writes `model` to `name`.vox under `dir`, placed in MagicaVoxel's scene as `sidecar` says, and
/// `sidecar` as its .json beside it
pub fn write_sidecar(dir: &Path, name: &str, model: &Model, mut sidecar: Sidecar) -> Result<PathBuf, String> {
    let path = file_of(dir, name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let grid = Grid { size: model.size, voxels: model.voxels.clone(), origin: sidecar.origin };
    let bytes = vox::write(&grid, &palette());
    sidecar.written = Some((bytes.len() as u64, hash_of(&bytes)));
    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let json = serde_json::to_string_pretty(&sidecar).map_err(|e| e.to_string())?;
    std::fs::write(path.with_extension("json"), json).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// Writes a jewel's model to `path`, in its own colours rather than the materials', with its .json
/// beside it, see jewels::export()
pub(crate) fn write_jewel(path: &Path, grid: &Grid, palette: &[[u8; 4]; 256], what: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let bytes = vox::write(grid, palette);
    let sidecar = Sidecar {
        what: what.into(),
        voxels_per_meter: MODEL_RES,
        size: grid.size,
        pivot: [-grid.origin[0] as f64, -grid.origin[1] as f64, -grid.origin[2] as f64],
        anchor: None,
        origin: grid.origin,
        points: BTreeMap::new(),
        note: "A jewel, drawn only as its icon, in the inventory, the quick bar and a weapon's card. Its colours are its own, not materials: 1 its body, 2 its lighter facets, 3 gold, 4 white sparks. Change them in MagicaVoxel's palette.".into(),
        written: Some((bytes.len() as u64, hash_of(&bytes))),
    };
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let json = serde_json::to_string_pretty(&sidecar).map_err(|e| e.to_string())?;
    std::fs::write(path.with_extension("json"), json).map_err(|e| format!("{}: {e}", path.display()))
}

/// Writes palette.txt, saying which colour index is which material, and palette.png, which
/// MagicaVoxel can load as its palette
pub fn write_palette(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut text = String::from(
        "# Colour index = material id, see src\\worldgen.rs (material) and the palette in shaders\\traverse.comp.\n\
         # Paint with these colours. Colour indices past the last material, and colours changed in MagicaVoxel's\n\
         # palette, become the material nearest their colour when the game reads the file.\n\
         # index  rgb      name\n",
    );
    for (i, m) in materials().iter().enumerate().skip(1) {
        text += &format!("{i:>5}  {:02x}{:02x}{:02x}   {}\n", m.rgb[0], m.rgb[1], m.rgb[2], m.name);
    }
    std::fs::write(dir.join("palette.txt"), text).map_err(|e| e.to_string())?;
    // MagicaVoxel's palette images are 256 by 1, colour index i at x = i - 1
    let p = palette();
    let pixels: Vec<u8> = (0..256).flat_map(|x| p[(x + 1) % 256]).collect();
    image::save_buffer(dir.join("palette.png"), &pixels, 256, 1, image::ColorType::Rgba8).map_err(|e| e.to_string())
}

/// What a look is called in the lists, with its kind where that tells it apart
pub fn look_title(look: Look) -> String {
    let name = look.name();
    let mut t = name[..1].to_uppercase() + &name[1..];
    if matches!(look, Look::GrayCat | Look::GingerCat | Look::Chest | Look::OpenChest | Look::Villager | Look::Townswoman) {
        t += &format!(" ({look:?})");
    }
    t
}

/// What's said in a look's .json about how its frames go
pub const LOOK_NOTE: &str = "The standing frame. The game draws this look in baked frames, walking and attacking: what you change \
    here is changed in every other frame the same way, where that frame has the same there. Export with --export-frames \
    for a file per frame, which replaces that frame outright.";

/// Everything exported, a line each, and the looks drawn from another's file
pub struct Exported {
    pub lines: Vec<String>,
}

/// Writes every model the code builds to `dir`, see the module's comment: the looks' standing
/// frames, or with `all_frames` every frame they're drawn in; the dragons' and great serpents'
/// pieces; the wings; and every part of every family of weapons and tools
pub fn export_all(dir: &Path, all_frames: bool) -> Result<Exported, String> {
    CODE_ONLY.store(true, Ordering::Relaxed);
    let models = animals::animal_models();
    let wings = crate::wings::models();
    let library = Library::load();
    CODE_ONLY.store(false, Ordering::Relaxed);
    write_palette(dir)?;
    let mut lines = Vec::new();
    let n = frames();
    let frames_of = |l: Look| -> Vec<&Model> {
        let k = look_model(l, 0);
        models[k..k + n].iter().collect()
    };
    // Looks
    let mut skins: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>> = BTreeMap::new();
    for &look in &Look::ALL {
        let frames = frames_of(look);
        if is_empty(frames[0]) {
            continue;
        }
        let folder = look_folder(look);
        // Baked from pieces: its pieces, each its own file, and its frames only if asked for
        if let Some(rig) = crate::baked::rig(look) {
            if let Some((from, map)) = crate::baked::skin_of(look) {
                let swaps = (1..256).filter(|&i| map[i] != i as u8).map(|i| (name_of(i as u8), name_of(map[i]))).collect();
                skins.entry(look_folder(from)).or_default().insert(folder.clone(), swaps);
                lines.push(format!("{folder}: drawn from {}'s pieces in its own colours, see {}/skins.json", look_folder(from), look_folder(from)));
                continue;
            }
            for (p, piece) in rig.pieces.iter().enumerate() {
                if piece.is_empty() {
                    continue;
                }
                let name = format!("{folder}/{}", rig.bones[p].name);
                let what = format!("{}, {}", look_title(look), rig.bones[p].name.replace('_', " "));
                write_model(dir, &name, &piece.model, MODEL_RES, Some(piece.anchor()), &what, PIECE_NOTE)?;
                lines.push(format!("{name}.vox: {what}"));
            }
            if all_frames {
                for (f, m) in frames.iter().enumerate().skip(1).filter(|(_, m)| !is_empty(m)) {
                    let name = format!("{folder}/{}", frame_name(f));
                    write_model(dir, &name, m, MODEL_RES, None, &format!("{}, frame {f}: {}", look_title(look), frame_name(f)), "Replaces this frame outright.")?;
                    lines.push(format!("{name}.vox"));
                }
            }
            continue;
        }
        if let Some((from, map)) = skin_of(look, &frames_of) {
            let swaps = (1..256).filter(|&i| map[i] != i as u8).map(|i| (name_of(i as u8), name_of(map[i]))).collect();
            skins.entry(look_folder(from)).or_default().insert(folder.clone(), swaps);
            lines.push(format!("{folder}: drawn from {}/base.vox in its own colours, see {}/skins.json", look_folder(from), look_folder(from)));
            continue;
        }
        let res = look_res(look);
        let shown = frames.iter().filter(|m| !is_empty(m)).count();
        let what = format!("{}, standing frame (1 of {shown} frames)", look_title(look));
        write_model(dir, &format!("{folder}/base"), frames[0], res, None, &what, if shown > 1 { LOOK_NOTE } else { "" })?;
        lines.push(format!("{folder}/base.vox: {what}"));
        if all_frames {
            for (f, m) in frames.iter().enumerate().skip(1).filter(|(_, m)| !is_empty(m)) {
                let name = format!("{folder}/{}", frame_name(f));
                write_model(dir, &name, m, res, None, &format!("{}, frame {f}: {}", look_title(look), frame_name(f)), "Replaces this frame outright.")?;
                lines.push(format!("{name}.vox"));
            }
        }
    }
    for (folder, members) in &skins {
        let json = serde_json::to_string_pretty(members).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(folder).join("skins.json"), json).map_err(|e| e.to_string())?;
    }
    // Dragons' and jointed bosses' pieces, those alike but for colour once
    let dragon_name = |k: usize, p: usize| format!("{}, {}", crate::dragons::name(crate::dragons::KINDS[k]), dragon_piece_name(p).replace('_', " "));
    export_jointed(dir, &models, animals::DRAGON_MODELS_AT, &crate::dragons::KINDS, dragon_pieces(), &crate::dragons::anchors(), &dragon_file, &dragon_name, &mut lines)?;
    let boss_name = |k: usize, p: usize| format!("{}, {}", look_title(crate::rig::KINDS[k]), crate::rig::edit::piece_name(crate::rig::KINDS[k], p).replace('_', " "));
    export_jointed(dir, &models, animals::RIG_MODELS_AT, &crate::rig::KINDS, crate::rig::PIECES, &crate::rig::edit::anchors(), &boss_file, &boss_name, &mut lines)?;
    let avian_name = |k: usize, p: usize| format!("{}, {}", look_title(crate::avians::KINDS[k]), crate::avians::PIECE_NAMES[p].replace('_', " "));
    export_jointed(dir, &models, animals::WINGED_MODELS_AT, &crate::avians::KINDS, avian_pieces(), &crate::avians::anchors(), &avian_file, &avian_name, &mut lines)?;
    // What dragons breathe and avians throw, the effects of fighting, the wards and the motes the
    // wings leave behind
    let dragon_fx = animals::DRAGON_MODELS_AT + crate::dragons::KINDS.len() * dragon_pieces();
    for fx in 0..crate::dragons::MODELS - crate::dragons::KINDS.len() * dragon_pieces() {
        let name = dragon_fx_file(fx).unwrap();
        write_model(dir, &name, &models[dragon_fx + fx], MODEL_RES, Some(crate::dragons::fx_shape(fx).0), &name, "Placed as the code places it.")?;
        lines.push(format!("{name}.vox: what dragons breathe and leave burning"));
    }
    let avian_fx = animals::WINGED_MODELS_AT + crate::avians::KINDS.len() * avian_pieces();
    let winged_anchors = crate::avians::anchors();
    for fx in 0..crate::avians::MODELS - crate::avians::KINDS.len() * avian_pieces() {
        let name = avian_fx_file(fx).unwrap();
        let i = crate::avians::KINDS.len() * avian_pieces() + fx;
        write_model(dir, &name, &models[avian_fx + fx], MODEL_RES, winged_anchors.get(i).copied(), &name, "Placed as the code places it.")?;
        lines.push(format!("{name}.vox: what avians throw and call down"));
    }
    let fixed_note = "Placed by its anchor as the code places it, and grows only round it. Its voxels a meter are the code's, not known here.";
    let effect_list = crate::weapons::effect_models();
    count_effects(effect_list.len());
    for (i, (m, a)) in effect_list.iter().enumerate() {
        let name = effect_file(i).unwrap();
        write_model(dir, &name, m, MODEL_RES, Some(*a), "an arrow, bolt, shot, spell, spark, smoke, pool or the player's arm", fixed_note)?;
        lines.push(format!("{name}.vox: {}", if name.starts_with("ammo/") { "special ammo in flight" } else { "an effect of fighting" }));
    }
    for (i, (m, a)) in crate::guardians::models().iter().enumerate() {
        let name = ward_file(i).unwrap();
        write_model(dir, &name, m, MODEL_RES, Some(*a), "a ward round a raid boss", fixed_note)?;
        lines.push(format!("{name}.vox: ward {i}"));
    }
    for (i, (m, a)) in wings.iter().enumerate().skip(crate::wings::KINDS.len()) {
        let name = wing_file(i).unwrap();
        write_model(dir, &name, m, WING_RES, Some(*a), "a mote of a wing's trail", "")?;
        lines.push(format!("{name}.vox: a mote of a wing's trail"));
    }
    // Great serpents' pieces
    let first = animals::ANIMAL_MODELS - crate::serpents::PART_MODELS;
    let serpent_anchors = crate::serpents::anchors();
    for i in 0..crate::serpents::PART_MODELS {
        let name = serpent_file(i).unwrap();
        let what = format!("{}, {}", crate::serpents::GREATS[i / 3].look.name(), SERPENT_PARTS[i % 3]);
        write_model(dir, &name, &models[first + i], MODEL_RES, Some(serpent_anchors[i]), &what, "A piece placed along the serpent's body: the pivot is the point it's made round.")?;
        lines.push(format!("{name}.vox: {what}"));
    }
    // The krakens' pieces
    let first = animals::KRAKEN_MODELS_AT;
    let kraken_anchors = crate::bosses::kraken::anchors();
    for i in 0..crate::bosses::kraken::MODELS {
        let name = kraken_file(i).unwrap();
        let parts = crate::bosses::kraken::PART_NAMES.len();
        let what = format!("{}, {}", crate::bosses::kraken::LOOKS[i / parts].name(), crate::bosses::kraken::PART_NAMES[i % parts]);
        write_model(dir, &name, &models[first + i], MODEL_RES, Some(kraken_anchors[i]), &what, "A piece placed along the kraken's body and arms: the pivot is the point it's made round.")?;
        lines.push(format!("{name}.vox: {what}"));
    }
    // Wings
    for (i, kind) in crate::wings::KINDS.iter().enumerate() {
        let (m, a) = &wings[i];
        let name = wing_file(i).unwrap();
        write_model(dir, &name, m, WING_RES, Some(*a), kind.name, "The right wing; the left is it mirrored.")?;
        lines.push(format!("{name}.vox: {}", kind.name));
    }
    // The player's body, in the looks a new character has
    let looks = crate::character::Looks::default();
    for (p, name) in crate::character::body::PIECE_NAMES.iter().enumerate() {
        let (m, a) = crate::character::body::piece(p, &looks);
        let file = player_file(p).unwrap();
        let what = format!("The player, {}", name.replace('_', " "));
        write_model(dir, &file, &m, crate::character::body::BODY_RES, Some(a), &what, PLAYER_NOTE)?;
        lines.push(format!("{file}.vox: {what}"));
    }
    // Weapons' parts
    crate::weapon_parts::export(dir, &library, &mut lines)?;
    // Jewels, see jewels.rs
    crate::jewels::export(dir, &mut lines)?;
    // The trees' numbers, beside assets\models as the game reads them, see worldgen\trees.rs
    let trees = if dir == root() { crate::worldgen::trees_path() } else { dir.join("trees.json") };
    std::fs::write(&trees, serde_json::to_string_pretty(&crate::worldgen::TreeBook::default()).map_err(|e| e.to_string())? + "\n").map_err(|e| e.to_string())?;
    lines.push(format!("{}: the trees' numbers", trees.display()));
    let index = lines.join("\n") + "\n";
    std::fs::write(dir.join("exported.txt"), &index).map_err(|e| e.to_string())?;
    Ok(Exported { lines })
}

/// Writes the pieces of jointed things, `pieces` to each of `kinds`, their models from `first`
/// among `models`: those drawn alike but for their colours once
#[allow(clippy::too_many_arguments)]
fn export_jointed(
    dir: &Path,
    models: &[Model],
    first: usize,
    kinds: &[Look],
    pieces: usize,
    anchors: &[Vector3<f64>],
    file: &dyn Fn(usize) -> Option<String>,
    what: &dyn Fn(usize, usize) -> String,
    lines: &mut Vec<String>,
) -> Result<(), String> {
    for k in 0..kinds.len() {
        let mine: Vec<&Model> = (0..pieces).map(|p| &models[first + k * pieces + p]).collect();
        let shared = (0..k).find(|&j| {
            let theirs: Vec<&Model> = (0..pieces).map(|p| &models[first + j * pieces + p]).collect();
            skin_map(&theirs, &mine).is_some()
        });
        if let Some(j) = shared {
            let folder = |k: usize| file(k * pieces).and_then(|f| f.rsplit_once('/').map(|(d, _)| d.to_string())).unwrap_or_default();
            lines.push(format!("{}: drawn from {}'s files in its own colours", folder(k), folder(j)));
            continue;
        }
        for (p, m) in mine.iter().enumerate() {
            if is_empty(m) {
                continue;
            }
            let name = file(k * pieces + p).unwrap();
            let what = what(k, p);
            write_model(dir, &name, m, MODEL_RES, Some(anchors[k * pieces + p]), &what, "A jointed piece: the pivot is its joint.")?;
            lines.push(format!("{name}.vox: {what}"));
        }
    }
    Ok(())
}

fn name_of(m: u8) -> String {
    materials().get(m as usize).map_or_else(|| format!("{m}"), |i| format!("{m} {}", i.name))
}

/// `innert --export-models <dir>`: writes every model, without opening a window, see export_all()
pub fn export_from_args() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == "--export-models")?;
    let dir = args.get(i + 1).filter(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(root);
    let all_frames = args.iter().any(|a| a == "--export-frames");
    attach_console();
    let start = std::time::Instant::now();
    Some(match export_all(&dir, all_frames) {
        Ok(done) => {
            println!("Wrote {} models to {} in {:.1} s, listed in exported.txt", done.lines.len(), dir.display(), start.elapsed().as_secs_f64());
            0
        }
        Err(e) => {
            eprintln!("Couldn't export the models: {e}");
            1
        }
    })
}

/// Prints to the terminal the game was started from, as a build without a console of its own has
/// nowhere to print otherwise
fn attach_console() {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn AttachConsole(process: u32) -> i32;
        }
        unsafe {
            AttachConsole(u32::MAX);
        }
    }
}

// Reading files again while the game runs

/// What the game's models are made of, for working out the files read again away from the game:
/// its models, the looks', dragons' and great serpents' first, then the rest as model_words()
/// packs them, the wings among them
pub struct Lists {
    pub base: Arc<Vec<Model>>,
    /// The arrows, spells and leaves, between the base and the wings: the first `effects` of them
    /// the effects, see weapons::effect_models(), with their anchors
    pub between: Vec<Model>,
    pub effects: usize,
    pub effect_anchors: Vec<Vector3<f64>>,
    pub wings: Vec<(Model, Vector3<f64>)>,
    /// The wards, after the wings, with their anchors, and anything after them
    pub after: Vec<Model>,
    pub ward_anchors: Vec<Vector3<f64>>,
    /// The weapons carried: each one's family, parts and seed, to make again
    pub weapons: Vec<(usize, Vec<usize>, u32)>,
}

/// What a reading of the files comes back with
pub struct Reloaded<T> {
    /// Whatever the models packed were made into, as the GPU's buffer of them
    pub made: Option<T>,
    pub wings: Option<Vec<(Model, Vector3<f64>)>>,
    /// The weapons carried made again, when their parts' files changed, with what they were made
    /// from, see Game::set_weapon_looks()
    pub weapons: Option<Vec<(usize, Vec<usize>, u32, crate::weapons::WeaponLook)>>,
}

/// Watches assets\models, and when a file there's saved works out the models again on a thread of
/// its own: packed for the shader and made into the GPU's buffer there too, so the game only
/// swaps it in, see poll()
pub struct Reloader<T> {
    changed: Arc<Mutex<HashSet<PathBuf>>>,
    _watcher: Option<notify::RecommendedWatcher>,
    job: Option<Receiver<Reloaded<T>>>,
    /// The base models with every file read since the game started, by index
    over: Arc<Mutex<HashMap<usize, Model>>>,
    /// When the last change was seen: files are read once they've been quiet a moment, as an
    /// editor saving writes them in several goes
    last: Option<std::time::Instant>,
}

impl<T: Send + 'static> Reloader<T> {
    pub fn new() -> Self {
        let changed: Arc<Mutex<HashSet<PathBuf>>> = Default::default();
        let dir = root();
        let watcher = (enabled() && dir.parent().is_some_and(|p| p.exists())).then(|| {
            use notify::Watcher;
            let seen = changed.clone();
            let mut w = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res
                    && (event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove())
                {
                    // The trees' numbers or hand-made trees saved: the next strips of the world
                    // grow theirs from them, see worldgen	rees.rs
                    let trees = root().join("trees");
                    if event.paths.iter().any(|p| p.ends_with("trees.json") || p.starts_with(&trees)) {
                        crate::worldgen::reload_trees();
                    }
                    // An animation saved: read again as it's next wanted, see anim.rs
                    let anims = crate::anim::root();
                    if event.paths.iter().any(|p| p.starts_with(&anims)) {
                        crate::anim::forget();
                    }
                    let mut c = seen.lock().unwrap_or_else(|e| e.into_inner());
                    c.extend(event.paths.into_iter().filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vox") || x.eq_ignore_ascii_case("json"))));
                }
            })
            .ok()?;
            // The folder above, so models\ is seen when it's first made
            w.watch(dir.parent()?, notify::RecursiveMode::Recursive).ok()?;
            Some(w)
        });
        Reloader { changed, _watcher: watcher.flatten(), job: None, over: Default::default(), last: None }
    }

    /// Reads the files again once they've changed and been quiet a moment: `lists` gives what's
    /// needed to do it, `make` turns the models packed into what's swapped in, done on that thread
    /// too. What's done comes back here a later frame.
    pub fn poll(&mut self, lists: impl FnOnce() -> Lists, make: impl FnOnce(Vec<u32>) -> T + Send + 'static) -> Option<Reloaded<T>> {
        if let Some(rx) = &self.job {
            return match rx.try_recv() {
                Ok(done) => {
                    self.job = None;
                    Some(done)
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(_) => {
                    self.job = None;
                    None
                }
            };
        }
        let pending = !self.changed.lock().unwrap_or_else(|e| e.into_inner()).is_empty();
        if !pending {
            return None;
        }
        let now = std::time::Instant::now();
        // A moment after the first change, so a file's read once it's all written
        let first = *self.last.get_or_insert(now);
        if now.duration_since(first).as_secs_f64() < 0.25 {
            return None;
        }
        self.last = None;
        let paths: Vec<PathBuf> = self.changed.lock().unwrap_or_else(|e| e.into_inner()).drain().collect();
        self.start(paths, lists(), make);
        None
    }

    /// Reads them again now, whether they've changed or not, as the editor does on asking
    pub fn reload_all(&mut self, lists: Lists, make: impl FnOnce(Vec<u32>) -> T + Send + 'static) {
        if self.job.is_none() {
            self.start(vec![root()], lists, make);
        }
    }

    fn start(&mut self, paths: Vec<PathBuf>, lists: Lists, make: impl FnOnce(Vec<u32>) -> T + Send + 'static) {
        let (tx, rx) = channel();
        let over = self.over.clone();
        self.job = Some(rx);
        std::thread::Builder::new()
            .name("reading model files".into())
            .spawn(move || {
                let start = std::time::Instant::now();
                let result = reread(&paths, &lists, &over);
                let over = over.lock().unwrap_or_else(|e| e.into_inner());
                let wings = result.wings;
                let wing_list: &Vec<(Model, Vector3<f64>)> = wings.as_ref().unwrap_or(&lists.wings);
                let between = result.between.as_ref().unwrap_or(&lists.between);
                let after = result.after.as_ref().unwrap_or(&lists.after);
                let repack = result.models || wings.is_some() || result.between.is_some() || result.after.is_some();
                let made = repack.then(|| {
                    let models: Vec<&Model> = (0..lists.base.len())
                        .map(|i| over.get(&i).unwrap_or(&lists.base[i]))
                        .chain(between.iter())
                        .chain(wing_list.iter().map(|(m, _)| m))
                        .chain(after.iter())
                        .collect();
                    make(animals::pack_models(&models))
                });
                drop(over);
                let weapons = result.weapons.then(|| {
                    let library = Library::load();
                    lists.weapons.iter().map(|(f, parts, seed)| (*f, parts.clone(), *seed, crate::weapons::Weapon::assemble(&library, *f, parts.clone(), *seed).look)).collect()
                });
                if made.is_some() || wings.is_some() || weapons.is_some() {
                    log(format!("Model files read again in {:.0} ms", start.elapsed().as_secs_f64() * 1000.0));
                }
                let _ = tx.send(Reloaded { made, wings, weapons });
            })
            .ok();
    }
}

struct Reread {
    /// Whether any of the base models changed
    models: bool,
    wings: Option<Vec<(Model, Vector3<f64>)>>,
    /// The effects and leaves, and the wards and the rest after the wings, made again
    between: Option<Vec<Model>>,
    after: Option<Vec<Model>>,
    weapons: bool,
}

/// Works out again the models the files at `paths` are of, into `over`
fn reread(paths: &[PathBuf], lists: &Lists, over: &Mutex<HashMap<usize, Model>>) -> Reread {
    let dir = root();
    *EDITED.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let files = edited(&dir);
    // The files changed, by name; the whole folder for all of them
    let names: HashSet<String> = paths
        .iter()
        .flat_map(|p| {
            if p == &dir {
                files.iter().cloned().collect::<Vec<_>>()
            } else {
                p.with_extension("").strip_prefix(&dir).map(|r| vec![r.to_string_lossy().replace('\\', "/")]).unwrap_or_default()
            }
        })
        .collect();
    // Animation files saved, by their path from assets\anims without .json, see src\anim.rs
    let anim_root = crate::anim::root();
    let anims: Vec<String> = paths.iter().filter_map(|p| p.with_extension("").strip_prefix(&anim_root).ok().map(|r| r.to_string_lossy().replace('\\', "/"))).collect();
    let everything = paths.iter().any(|p| p == &dir);
    let mut out = Reread { models: false, wings: None, between: None, after: None, weapons: false };
    if names.is_empty() && anims.is_empty() && !everything {
        return out;
    }
    let n = frames();
    let base = &lists.base;
    // The code's model `i`: kept aside if it was replaced at startup
    let code_of = |i: usize, s: &State| -> Model { s.code.get(&i).map(copy).unwrap_or_else(|| copy(&base[i])) };
    let mut s = state();
    let mut new: Vec<(usize, Model)> = Vec::new();
    // Looks baked from pieces, baked again from their pieces' files and their animation files, see
    // src\baked.rs: any whose files, or the files of the look they share their shape with, or whose
    // clips, its own or its build's, were saved
    let in_folder = |l: Look| names.iter().any(|f| f.split('/').next() == Some(look_folder(l).as_str()));
    for &look in &crate::baked::KINDS {
        let clip = anims.iter().any(|a| crate::baked::clip_file_of(look, a));
        let shared = crate::baked::skin_of(look).is_some_and(|(from, _)| in_folder(from));
        if !(everything || clip || shared || in_folder(look)) {
            continue;
        }
        let Some(rig) = crate::baked::rig(look) else { continue };
        let (frames, pieces) = baked_look(&dir, &files, look).unwrap_or_else(|| {
            crate::baked::laid_out(look, &rig.pieces);
            (rig.frames.clone(), (0..crate::baked::PIECES).map(|p| rig.pieces.get(p).map_or_else(|| air([1, 1, 1]), |piece| piece.model.clone())).collect())
        });
        new.extend(frames.into_iter().enumerate().map(|(f, m)| (look_model(look, f), m)));
        new.extend(pieces.into_iter().enumerate().filter_map(|(p, m)| Some((baked_model(look, p)?, m))));
    }
    // Looks, and those drawn from them in their own colours
    let looks: Vec<Look> = Look::ALL.iter().copied().filter(|&l| !crate::baked::baked(l) && in_folder(l)).collect();
    if !looks.is_empty() {
        let code: Vec<Vec<Model>> = Look::ALL.iter().map(|&l| (0..n).map(|f| code_of(look_model(l, f), &s)).collect()).collect();
        let frames_of = |l: Look| -> Vec<&Model> { code[Look::ALL.iter().position(|&k| k == l).unwrap()].iter().collect() };
        for &look in &looks {
            let frames = look_frames(&dir, &files, look, &frames_of(look));
            for &member in &Look::ALL {
                if member != look
                    && !has_files(&files, member)
                    && let Some((from, map)) = skin_of(member, &frames_of)
                    && from == look
                {
                    new.extend(frames.iter().enumerate().map(|(f, m)| (look_model(member, f), recolor(m, &map))));
                }
            }
            new.extend(frames.into_iter().enumerate().map(|(f, m)| (look_model(look, f), m)));
        }
    }
    // Dragons' and great serpents' pieces, each read whole into the size it had at startup, which
    // the code's kept since; they're only freer to grow when the game starts again
    let pieces = dragon_pieces();
    let first_dragon = animals::DRAGON_MODELS_AT;
    let first_serpent = animals::ANIMAL_MODELS - crate::serpents::PART_MODELS;
    let first_boss = animals::RIG_MODELS_AT;
    let first_winged = animals::WINGED_MODELS_AT;
    let (dragon_anchors, serpent_anchors, boss_anchors, winged_anchors) = (crate::dragons::anchors(), crate::serpents::anchors(), crate::rig::edit::anchors(), crate::avians::anchors());
    let anchor_of = |i: usize| {
        if i >= first_serpent {
            serpent_anchors.get(i - first_serpent).copied()
        } else if i >= first_winged {
            winged_anchors.get(i - first_winged).copied()
        } else if i >= first_dragon {
            dragon_anchors.get(i - first_dragon).copied()
        } else {
            boss_anchors.get(i - first_boss).copied()
        }
    };
    let whole = |i: usize, name: Option<String>, s: &State, new: &mut Vec<(usize, Model)>| {
        let Some(name) = name.filter(|n| names.contains(n)) else { return };
        let code = code_of(i, s);
        let now = &base[i];
        let model = if files.contains(&name) {
            match read_edit(&file_of(&dir, &name), pivot_of(now.size, anchor_of(i), MODEL_RES)) {
                Ok(edit) => conform(&edit, now.size, Grow::Fixed { cap: 1023 }).0,
                Err(e) => {
                    log(format!("{name}.vox: {e}"));
                    return;
                }
            }
        } else {
            code
        };
        new.push((i, model));
    };
    for i in 0..crate::dragons::KINDS.len() * pieces {
        whole(first_dragon + i, dragon_file(i), &s, &mut new);
    }
    for i in 0..crate::rig::MODELS {
        whole(first_boss + i, boss_file(i), &s, &mut new);
    }
    for i in 0..crate::avians::KINDS.len() * avian_pieces() {
        whole(first_winged + i, avian_file(i), &s, &mut new);
    }
    let dragon_pieces_all = crate::dragons::KINDS.len() * pieces;
    for fx in 0..crate::dragons::MODELS - dragon_pieces_all {
        whole(first_dragon + dragon_pieces_all + fx, dragon_fx_file(fx), &s, &mut new);
    }
    let avian_pieces_all = crate::avians::KINDS.len() * avian_pieces();
    for fx in 0..crate::avians::MODELS - avian_pieces_all {
        whole(first_winged + avian_pieces_all + fx, avian_fx_file(fx), &s, &mut new);
    }
    for i in 0..crate::serpents::PART_MODELS {
        whole(first_serpent + i, serpent_file(i), &s, &mut new);
    }
    // Keep the code's models of those replaced, to work out from again
    for (i, _) in &new {
        if !s.code.contains_key(i) {
            let kept = copy(&base[*i]);
            s.code.insert(*i, kept);
        }
    }
    out.models = !new.is_empty();
    if out.models {
        let mut over = over.lock().unwrap_or_else(|e| e.into_inner());
        for (i, m) in new {
            over.insert(i, m);
        }
        log(format!("Read again: {}", names.iter().cloned().collect::<Vec<_>>().join(", ")));
    }
    // Wings
    if names.iter().any(|f| f.starts_with("wings/")) {
        let code = s.code_wings.get_or_insert_with(|| lists.wings.iter().map(|(m, a)| (copy(m), *a)).collect());
        let mut made: Vec<(Model, Vector3<f64>)> = code.iter().map(|(m, a)| (copy(m), *a)).collect();
        drop(s);
        apply_anchored(&mut made, WING_RES, &wing_file);
        out.wings = Some(made);
    } else {
        drop(s);
    }
    // The effects of fighting and the wards, each list read whole again from the code's
    if names.iter().any(|f| f.starts_with("effects/") || f.starts_with("ammo/")) {
        let mut s = state();
        let code = s.code_effects.get_or_insert_with(|| lists.between[..lists.effects].iter().map(|m| (copy(m), Vector3::zeros())).collect());
        let mut made: Vec<(Model, Vector3<f64>)> = code.iter().map(|(m, a)| (copy(m), *a)).collect();
        let anchors = &lists.effect_anchors;
        for (k, (_, a)) in made.iter_mut().enumerate() {
            if let Some(&anchor) = anchors.get(k) {
                *a = anchor;
            }
        }
        let files = edited(&dir);
        for (i, (model, anchor)) in made.iter_mut().enumerate() {
            let name = effect_file(i).unwrap();
            if files.contains(&name)
                && let Ok(edit) = read_edit(&file_of(&dir, &name), pivot_of(model.size, Some(*anchor), MODEL_RES))
            {
                *model = conform(&edit, model.size, Grow::Fixed { cap: 1023 }).0;
            }
        }
        out.between = Some(made.into_iter().map(|(m, _)| m).chain(lists.between[lists.effects..].iter().map(copy)).collect());
    }
    if names.iter().any(|f| f.starts_with("wards/")) {
        let mut s = state();
        let code = s.code_wards.get_or_insert_with(|| lists.after.iter().zip(&lists.ward_anchors).map(|(m, a)| (copy(m), *a)).collect());
        let mut made: Vec<(Model, Vector3<f64>)> = code.iter().map(|(m, a)| (copy(m), *a)).collect();
        let files = edited(&dir);
        for (i, (model, anchor)) in made.iter_mut().enumerate() {
            let name = ward_file(i).unwrap();
            if files.contains(&name)
                && let Ok(edit) = read_edit(&file_of(&dir, &name), pivot_of(model.size, Some(*anchor), MODEL_RES))
            {
                *model = conform(&edit, model.size, Grow::Fixed { cap: 1023 }).0;
            }
        }
        // The stage's floor after them, as model_lists() gives it
        out.after = Some(made.into_iter().map(|(m, _)| m).chain(lists.after[lists.ward_anchors.len()..].iter().map(copy)).collect());
    }
    // Weapons: their parts' files read again as they're next made
    if names.iter().any(|f| f.starts_with("weapons/")) {
        crate::weapon_parts::forget();
        out.weapons = true;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The palette has every material, the right colour at its index, and a file's colours read
    /// back as the same materials
    #[test]
    fn palette_is_the_materials() {
        use crate::worldgen::material::*;
        let m = materials();
        assert_eq!(m.len(), LAST as usize + 1, "a material for every id");
        assert_eq!(m[GLASS as usize].name, "glass");
        assert_eq!(m[CLOTH_BLUE as usize].name, "blue cloth");
        assert_eq!(m[TEMPLE_RED as usize].name, "red paint");
        let map = material_map(&palette());
        for i in 1..m.len() {
            assert_eq!(map[i], i as u8);
        }
        // A colour changed in the file becomes the nearest material
        let mut p = palette();
        p[3] = [250, 250, 255, 255];
        assert_eq!(material_map(&p)[3], SNOW);
    }

    /// A model exported and read back in is voxel for voxel the same, with the same anchor; and
    /// grown past its box in MagicaVoxel, it keeps its voxels where they were
    #[test]
    fn models_round_trip() {
        let dir = std::env::temp_dir().join(format!("innert-vox-test-{}", std::process::id()));
        let parts = [
            animals::ball([0.0, 0.0, 0.5], 0.3, crate::worldgen::material::STONE),
            animals::limb([0.1, 0.0, 0.2], [0.6, 0.2, 0.9], 0.08, 0.05, crate::worldgen::material::GOLD),
        ];
        let (model, anchor) = animals::voxelize(&parts, MODEL_RES);
        write_model(&dir, "test/piece", &model, MODEL_RES, Some(anchor), "a test", "").unwrap();
        let edit = read_edit(&file_of(&dir, "test/piece"), pivot_of(model.size, Some(anchor), MODEL_RES)).unwrap();
        let (back, lo) = conform(&edit, model.size, Grow::Free);
        assert_eq!(back.size, model.size);
        assert_eq!(back.voxels, model.voxels);
        assert!((moved_anchor(anchor, model.size, lo, back.size, MODEL_RES) - anchor).norm() < 1e-12);
        // The same as a look's frame, held by the middle of its feet
        let (back, _) = conform(&edit, model.size, Grow::Fixed { cap: 255 });
        assert_eq!(back.voxels, model.voxels);

        // A voxel added beyond the box, as MagicaVoxel writes it once the model's grown: the
        // scene keeps it where it is next to the rest
        let file = vox::read(&std::fs::read(file_of(&dir, "test/piece")).unwrap()).unwrap();
        let mut g = file.grid.clone();
        let size = [g.size[0] + 3, g.size[1], g.size[2] + 2];
        let mut voxels = vec![0u8; (size[0] * size[1] * size[2]) as usize];
        for z in 0..g.size[2] {
            for y in 0..g.size[1] {
                for x in 0..g.size[0] {
                    voxels[(x + 3 + (y + (z + 2) * size[1]) * size[0]) as usize] = g.get([x, y, z]);
                }
            }
        }
        voxels[0] = crate::worldgen::material::FLAME;
        g = Grid { size, voxels, origin: [g.origin[0] - 3, g.origin[1], g.origin[2] - 2] };
        std::fs::write(file_of(&dir, "test/piece"), vox::write(&g, &palette())).unwrap();
        let edit = read_edit(&file_of(&dir, "test/piece"), pivot_of(model.size, Some(anchor), MODEL_RES)).unwrap();
        let (grown, lo) = conform(&edit, model.size, Grow::Free);
        assert_eq!(lo, [-3, 0, -2]);
        assert_eq!(grown.size, [model.size[0] + 3, model.size[1], model.size[2] + 2]);
        assert_eq!(grown.voxels[0], crate::worldgen::material::FLAME);
        for z in 0..model.size[2] {
            for y in 0..model.size[1] {
                for x in 0..model.size[0] {
                    assert_eq!(at(&grown, [x as i32 + 3, y as i32, z as i32 + 2]), at(&model, [x as i32, y as i32, z as i32]));
                }
            }
        }
        // Its anchor moved so the old voxels are where they were in the world
        let moved = moved_anchor(anchor, model.size, lo, grown.size, MODEL_RES);
        let world = |a: Vector3<f64>, s: [u32; 3], p: [i32; 3]| a + Vector3::new((p[0] as f64 + 0.5 - s[0] as f64 * 0.5) / MODEL_RES, (p[1] as f64 + 0.5 - s[1] as f64 * 0.5) / MODEL_RES, (p[2] as f64 + 0.5) / MODEL_RES);
        assert!((world(anchor, model.size, [1, 2, 3]) - world(moved, grown.size, [4, 2, 5])).norm() < 1e-9);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A change to a standing frame is made in the other frames where they have the same there,
    /// and a frame that's moved keeps its own
    #[test]
    fn changes_follow_what_stays_put() {
        use crate::worldgen::material::*;
        let mut code0 = air([4, 4, 4]);
        code0.voxels[0] = STONE;
        code0.voxels[5] = WOOD;
        let mut other = air([4, 4, 4]);
        other.voxels[0] = STONE;
        other.voxels[5] = GOLD;
        let mut edited = copy(&code0);
        edited.voxels[0] = ICE;
        edited.voxels[5] = AIR;
        edited.voxels[10] = FLAME;
        let c = changes(&code0, &edited, [0, 0, 0]);
        let (out, _) = apply_changes(&other, &c, [0, 0, 0], Grow::Fixed { cap: 255 });
        assert_eq!(out.voxels[0], ICE, "the same stone there, so it changes");
        assert_eq!(out.voxels[5], GOLD, "something else there, so it stays");
        assert_eq!(out.voxels[10], FLAME, "air there, so it's added");
    }

    /// Every model the game builds, exported and read back as the game reads it at startup, comes
    /// out voxel for voxel as the code made it: the looks', dragons', serpents', wings' and weapons'
    #[test]
    fn every_model_round_trips() {
        let dir = std::env::temp_dir().join(format!("innert-export-test-{}", std::process::id()));
        export_all(&dir, false).unwrap();
        let files = scan(&dir);
        assert!(files.contains("merchant/base"), "the merchant's standing frame");
        assert!(!files.contains("wolf/base"), "the wolf's baked from pieces");
        assert!(files.contains("dragon/ashwing/head"));
        assert!(files.contains("wings/gull"));
        CODE_ONLY.store(true, Ordering::Relaxed);
        let models = animals::animal_models();
        let wings = crate::wings::models();
        let _library = Library::load();
        CODE_ONLY.store(false, Ordering::Relaxed);
        let n = frames();
        for &look in &Look::ALL {
            if !has_files(&files, look) || crate::baked::baked(look) {
                continue;
            }
            let k = look_model(look, 0);
            let code: Vec<&Model> = models[k..k + n].iter().collect();
            let back = look_frames(&dir, &files, look, &code);
            for f in 0..n {
                assert_eq!(back[f].size, code[f].size, "{look:?} frame {f}");
                assert!(back[f].voxels == code[f].voxels, "{look:?} frame {f}");
            }
        }
        // The looks baked from pieces: every piece read back as it was, and every frame baked from
        // them as the code bakes it, but for those drawn from another's pieces, which have none
        assert!(files.contains("armored_skeleton/blade") && files.contains("wolf/upper_fore_left"), "the baked looks' pieces");
        assert!(!files.iter().any(|f| f.starts_with("ginger_cat/")), "the ginger cat's drawn from the grey cat's");
        for &look in &crate::baked::KINDS {
            let rig = crate::baked::rig(look).unwrap();
            if crate::baked::skin_of(look).is_some() {
                continue;
            }
            assert!(crate::baked::files_of(look, &files).len() >= 4, "{look:?}'s pieces");
            let back = crate::baked::read_pieces(look, &dir, &files, &look_folder(look));
            for (p, (a, b)) in back.iter().zip(&rig.pieces).enumerate() {
                assert!(a.model.size == b.model.size && a.model.voxels == b.model.voxels && (a.anchor() - b.anchor()).norm() < 1e-9, "{look:?} piece {p}");
            }
            let frames = rig.bake(&back, None);
            for f in 0..n {
                assert!(frames[f].size == models[look_model(look, f)].size && frames[f].voxels == models[look_model(look, f)].voxels, "{look:?} frame {f}");
            }
        }
        // The jointed bosses', the dragons' and the avians' pieces, one after another among the
        // models, each with its effects after it, which have no files
        let dragon_fx = crate::dragons::MODELS - crate::dragons::KINDS.len() * dragon_pieces();
        let winged_fx = crate::avians::MODELS - crate::avians::KINDS.len() * avian_pieces();
        let anchors: Vec<Vector3<f64>> = crate::rig::edit::anchors()
            .into_iter()
            .chain(crate::dragons::anchors())
            .chain(crate::avians::anchors())
            .chain(std::iter::repeat_n(Vector3::zeros(), crate::baked::MODELS))
            .chain(crate::bosses::kraken::anchors())
            .collect();
        let base = animals::RIG_MODELS_AT;
        let names: Vec<Option<String>> = (0..crate::rig::MODELS)
            .map(boss_file)
            .chain((0..crate::dragons::KINDS.len() * dragon_pieces()).map(dragon_file))
            .chain((0..dragon_fx).map(|_| None))
            .chain((0..crate::avians::KINDS.len() * avian_pieces()).map(avian_file))
            .chain((0..winged_fx).map(|_| None))
            .chain((0..crate::baked::MODELS).map(|_| None))
            .chain((0..crate::bosses::kraken::MODELS).map(kraken_file))
            .collect();
        assert_eq!(names.len(), animals::ANIMAL_MODELS - crate::serpents::PART_MODELS - base);
        assert!(files.contains("avian/harpy/torso"), "the avians' pieces");
        assert!(files.contains("boss/golem/chest") && files.contains("dragon/drake/head"), "the bosses' and drakes' pieces");
        assert!(!files.iter().any(|f| f.starts_with("boss/frost_golem/")), "the frost golem's drawn from the golem's");
        for (i, name) in names.into_iter().enumerate() {
            let Some(name) = name else { continue };
            if !files.contains(&name) {
                continue;
            }
            let m = &models[base + i];
            let edit = read_edit(&file_of(&dir, &name), pivot_of(m.size, Some(anchors[i]), MODEL_RES)).unwrap();
            let (back, lo) = conform(&edit, m.size, Grow::Free);
            assert!(back.size == m.size && back.voxels == m.voxels && lo == [0, 0, 0], "{name}");
        }
        let fixed = |name: &str, m: &Model, a: Vector3<f64>| {
            let edit = read_edit(&file_of(&dir, name), pivot_of(m.size, Some(a), MODEL_RES)).unwrap();
            let (back, _) = conform(&edit, m.size, Grow::Fixed { cap: 1023 });
            assert!(back.size == m.size && back.voxels == m.voxels, "{name}");
        };
        let effect_list = crate::weapons::effect_models();
        count_effects(effect_list.len());
        for (i, (m, a)) in effect_list.iter().enumerate() {
            fixed(&effect_file(i).unwrap(), m, *a);
        }
        for (i, (m, a)) in crate::guardians::models().iter().enumerate() {
            fixed(&ward_file(i).unwrap(), m, *a);
        }
        let dragon_fx = animals::DRAGON_MODELS_AT + crate::dragons::KINDS.len() * dragon_pieces();
        for fx in 0..crate::dragons::MODELS - crate::dragons::KINDS.len() * dragon_pieces() {
            fixed(&dragon_fx_file(fx).unwrap(), &models[dragon_fx + fx], crate::dragons::fx_shape(fx).0);
        }
        for (i, (m, a)) in wings.iter().enumerate() {
            let name = wing_file(i).unwrap();
            let edit = read_edit(&file_of(&dir, &name), pivot_of(m.size, Some(*a), WING_RES)).unwrap();
            let (back, _) = conform(&edit, m.size, Grow::Free);
            assert!(back.voxels == m.voxels, "{name}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The golems share one file, drawn in each one's colours
    #[test]
    fn golems_share_a_shape() {
        CODE_ONLY.store(true, Ordering::Relaxed);
        let models = animals::animal_models();
        CODE_ONLY.store(false, Ordering::Relaxed);
        let n = frames();
        let frames_of = |l: Look| -> Vec<&Model> {
            let k = look_model(l, 0);
            models[k..k + n].iter().collect()
        };
        // A look drawn from another's is that one's frames with its materials swapped
        for &l in &Look::ALL {
            if let Some((from, map)) = skin_of(l, &frames_of) {
                for (a, b) in frames_of(from).iter().zip(frames_of(l)) {
                    assert!(recolor(a, &map).voxels == b.voxels, "{l:?} from {from:?}");
                }
            }
        }
        // The golems are jointed now: each one's pieces are the stone golem's in other colours
        let p = crate::rig::PIECES;
        let pieces = |l: Look| -> Vec<&Model> {
            let k = crate::rig::kind(l).unwrap();
            models[animals::RIG_MODELS_AT + k * p..animals::RIG_MODELS_AT + (k + 1) * p].iter().collect()
        };
        assert!(skin_map(&pieces(Look::Golem), &pieces(Look::FrostGolem)).is_some());
        assert!(skin_map(&pieces(Look::Golem), &pieces(Look::MagmaGolem)).is_some());
    }

    #[test]
    fn dragon_pieces_are_named() {
        assert_eq!(dragon_pieces(), DRAGON_PIECES.len());
    }

    /// With the model files the game ships with in assets\models, every model's just as the code
    /// makes it: none of them edited, so none of them drawn instead
    #[test]
    fn shipped_models_change_nothing() {
        let files = scan(&root());
        assert!(files.len() > 1000, "the models ship as files, {} of them", files.len());
        assert!(edited(&root()).iter().all(|f| f.starts_with("weapons/")), "files edited since they were exported: {:?}", edited(&root()).iter().filter(|f| !f.starts_with("weapons/")).collect::<Vec<_>>());
        CODE_ONLY.store(true, Ordering::Relaxed);
        let code = animals::animal_models();
        let code_wings = crate::wings::models();
        CODE_ONLY.store(false, Ordering::Relaxed);
        let shipped = apply_looks(animals::animal_models());
        assert_eq!(code.len(), shipped.len());
        assert!(code.iter().zip(&shipped).all(|(a, b)| a.size == b.size && a.voxels == b.voxels));
        let shipped_wings = wings(crate::wings::models());
        assert!(code_wings.iter().zip(&shipped_wings).all(|(a, b)| a.0.voxels == b.0.voxels && a.1 == b.1));
        let effects_code = crate::weapons::effect_models();
        let effects_shipped = effects(crate::weapons::effect_models());
        assert!(effects_code.iter().zip(&effects_shipped).all(|(a, b)| a.0.voxels == b.0.voxels && a.1 == b.1));
        // The player's body, as a new character looks
        let looks = crate::character::Looks::default();
        let body_code: Vec<(Model, Vector3<f64>)> = (0..crate::character::body::PIECES).map(|p| crate::character::body::piece(p, &looks)).collect();
        let body_shipped = crate::character::body::models(&looks);
        assert!(body_code.iter().zip(&body_shipped).all(|(a, b)| a.0.voxels == b.0.voxels && a.0.size == b.0.size), "the player's body's files are as the code makes it");
        assert!((0..crate::character::body::PIECES).all(|p| files.contains(&player_file(p).unwrap())), "the player's body ships as files");
        // And every weapon part shipped is the code's, so every weapon's built as it was
        let library = Library::load();
        for f in 0..library.families.len() {
            let parts = library.reference_parts(f);
            let code = crate::weapons::look_of(&library, f, &parts);
            let shipped = crate::weapon_parts::build(&library, f, &parts, || crate::weapons::look_of(&library, f, &parts));
            assert!(code.frames.iter().zip(&shipped.frames).all(|(a, b)| a.0.voxels == b.0.voxels), "{}", library.families[f].id);
        }
    }
}

#[cfg(test)]
mod timing {
    use super::*;

    #[test]
    #[ignore]
    fn how_long_files_take() {
        let t = std::time::Instant::now();
        let n = scan(&root()).len();
        println!("scan {n}: {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let e = edited(&root()).len();
        println!("edited {e}: {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let library = Library::load();
        for f in 0..library.families.len() {
            let parts = library.reference_parts(f);
            crate::weapon_parts::build(&library, f, &parts, || crate::weapons::look_of(&library, f, &parts));
        }
        println!("weapons with files: {:?}", t.elapsed());
        let t = std::time::Instant::now();
        for f in 0..library.families.len() {
            let parts = library.reference_parts(f);
            crate::weapons::look_of(&library, f, &parts);
        }
        println!("weapons without: {:?}", t.elapsed());
    }
}
