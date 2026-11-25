//! The model editor, its own app, innert-editor: any creature, boss, dragon, great serpent, pair of wings or
//! weapon shown alone on a plain floor high over the world, turned about with the mouse, its
//! animations played slowly with a bar to scrub through them, its pieces and bones listed, the one
//! picked lit up, and each piece opened in MagicaVoxel to change its shape, see src\assets.rs. A
//! jointed rig's animations are changed here too: a bone turned and moved, keyed on a timeline
//! with easing between keys, and saved to assets\anims, see src\anim.rs.
//!
//! It's the game's renderer on a studio of its own, a floor and nothing else, see Scene::Studio:
//! no landscape, nothing living, no saves. Files it saves reach a game running beside it at once.

use std::f64::consts::FRAC_PI_2;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use egui_winit_vulkano::egui::{self, Color32, RichText};
use nalgebra::{UnitQuaternion, Vector3};

use crate::anim::{self, Clip, Ease, Joint, Key, Rig};
use crate::animals::{self, AnimalInstance, HELD, Look, MODEL_RES, MOVING, Model, STILL};
use crate::assets;
use crate::game::Game;
use crate::weapon_parts;
use crate::weapons::Library;
use crate::worldgen::material::*;
use crate::worldgen::{World, is_passable};

/// Set in the model editor's own app, see editor_main()
static APP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether this is the model editor's own app, innert-editor, rather than the game
pub fn is_app() -> bool {
    APP.load(std::sync::atomic::Ordering::Relaxed)
}

/// Starts this as the model editor's own app, before anything else
pub fn set_app() {
    APP.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// How big the studio is, in voxels: 96 m across and 40 m high, room for the greatest dragon
pub const STUDIO_SIZE: [u32; 3] = [1536, 1536, 640];

/// The floor: squares this many voxels a side, each this many meters across, two shades of stone
const FLOOR_SIDE: u32 = 96;
const FLOOR_CELL: f64 = 0.5;

/// The editor's own models, drawn with the rest, see Game::stage_model(): the floor
pub fn stage_models() -> Vec<Model> {
    let n = FLOOR_SIDE;
    let voxels = (0..n * n).map(|i| if ((i % n) / 2 + (i / n) / 2) % 2 == 0 { STONE } else { GRAVEL }).collect();
    vec![Model { size: [n, n, 1], voxels }]
}

/// What can be shown
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Subject {
    /// An animal, monster or boss, or a thing, drawn in baked frames
    Look(Look),
    /// A dragon, drawn out of jointed pieces
    Dragon(Look),
    /// A boss drawn out of jointed pieces, see src\rig.rs
    Boss(Look),
    /// A monster, an animal or one of the folk of the towns, drawn in frames baked from jointed
    /// pieces, see src\baked.rs
    Baked(Look),
    /// An avian or a gargoyle, on the winged rig, see src\avians.rs
    Winged(Look),
    /// A great serpent's head, jaw and a piece of its body, by serpents::GREATS
    Serpent(usize),
    /// A kind of wings, by wings::KINDS
    Wings(usize),
    /// A family of weapons or tools, by Library::families
    Weapon(usize),
}

struct Entry {
    subject: Subject,
    title: String,
    group: &'static str,
}

/// One of a subject's animations: for a look drawn in frames, which frame it's in from when
struct Timeline {
    name: String,
    length: f64,
    frames: Vec<(f64, usize)>,
}

impl Timeline {
    fn frame_at(&self, t: f64) -> usize {
        self.frames.iter().rev().find(|(at, _)| *at <= t + 1e-9).map_or(0, |(_, f)| *f)
    }
}

/// What's asked of the editor by input this frame, see update()
#[derive(Default)]
pub struct Controls {
    /// The mouse moved with its left button held, and with its right, in pixels
    pub drag: (f64, f64),
    pub turn: (f64, f64),
    pub scroll: f64,
    /// Keys pressed: Q and E nudge the bone picked, X, Y and Z pick the axis, G switches between
    /// turning and moving, K keys it, Delete takes its key away, Space plays or pauses
    pub keys: Vec<char>,
}

pub struct Editor {
    pub open: bool,
    /// Whether the pointer's over the editor's windows, so dragging there doesn't turn the view
    pub over_ui: bool,
    entries: Vec<Entry>,
    pick: usize,
    search: String,
    library: Option<Library>,
    /// Where the floor's middle is, in voxels of the world
    stage: Vector3<f64>,
    yaw: f64,
    pitch: f64,
    /// How far the camera is from what it's looking at, and how high over the floor that is, in meters
    distance: f64,
    height: f64,
    clip: usize,
    time: f64,
    speed: f64,
    playing: bool,
    piece: Option<usize>,
    // A weapon: its parts by slot, which slot's picked, whether only that part's shown, and held
    // as in the hand
    parts: Vec<usize>,
    part_only: bool,
    first_person: bool,
    built: Option<(usize, Vec<usize>, bool, Option<usize>)>,
    preview_res: f64,
    preview_count: usize,
    /// The look each look's drawn from in its own colours, if it's drawn from another's, by Look::ALL
    skins: Vec<Option<Look>>,
    /// For a rig: the clip being changed, whether it's been changed since it was saved, whether
    /// it's saved for every rig of its kind, and how a bone's changed: turned or moved, about which axis
    clip_edit: Option<Clip>,
    dirty: bool,
    for_kind: bool,
    moving: bool,
    axis: usize,
    note: String,
    exporting: Option<Arc<Mutex<Option<String>>>>,
}

impl Default for Editor {
    fn default() -> Self {
        Editor {
            open: false,
            over_ui: false,
            entries: Vec::new(),
            pick: 0,
            search: String::new(),
            library: None,
            stage: Vector3::zeros(),
            yaw: 0.6,
            pitch: -0.3,
            distance: 8.0,
            height: 1.0,
            clip: 0,
            time: 0.0,
            speed: 0.5,
            playing: true,
            piece: None,
            parts: Vec::new(),
            part_only: false,
            first_person: false,
            built: None,
            preview_res: 192.0,
            preview_count: 1,
            skins: Vec::new(),
            clip_edit: None,
            dirty: false,
            for_kind: false,
            moving: false,
            axis: 1,
            note: String::new(),
            exporting: None,
        }
    }
}

/// The dragon rig's bones by the piece each is drawn with, see dragons::model_of()
fn dragon_piece(bone: usize) -> usize {
    match bone {
        _ if bone < 14 => bone,
        _ if bone < 18 => if bone - 14 < 2 { 14 } else { 16 },
        _ => if bone - 18 < 2 { 15 } else { 17 },
    }
}

/// The winged rig's bones by the piece each is drawn with, see avians::model_of()
fn winged_piece(bone: usize) -> usize {
    match bone {
        _ if bone < 8 => bone,
        _ if bone < 10 => 8,
        _ if bone < 12 => 9,
        _ if bone < 14 => 10,
        _ => 11,
    }
}

fn look_reach(look: Look) -> f64 {
    look.reach()
}

/// MagicaVoxel, if it's where it's usually put, or where `--set magicavoxel=<path>` says
pub fn magicavoxel() -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(set) = args.windows(2).filter(|w| w[0] == "--set").filter_map(|w| w[1].strip_prefix("magicavoxel=")).next() {
        return Some(PathBuf::from(set)).filter(|p| p.exists());
    }
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let mut places: Vec<PathBuf> = Vec::new();
    for base in [env("ProgramFiles"), env("ProgramFiles(x86)"), env("LOCALAPPDATA").map(|p| p.join("Programs")), env("USERPROFILE").map(|p| p.join("Downloads")), env("USERPROFILE").map(|p| p.join("Desktop")), env("USERPROFILE").map(|p| p.join("Documents")), Some(PathBuf::from("C:\\"))].into_iter().flatten() {
        places.push(base.join("MagicaVoxel").join("MagicaVoxel.exe"));
        // Unzipped where it was downloaded, in a folder named for its version
        if let Ok(dir) = std::fs::read_dir(&base) {
            for e in dir.flatten() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if name.starts_with("magicavoxel") && e.path().is_dir() {
                    places.push(e.path().join("MagicaVoxel.exe"));
                    if let Ok(inner) = std::fs::read_dir(e.path()) {
                        places.extend(inner.flatten().filter(|i| i.path().is_dir()).map(|i| i.path().join("MagicaVoxel.exe")));
                    }
                }
            }
        }
    }
    places.extend([env("ProgramFiles(x86)"), env("ProgramFiles")].into_iter().flatten().map(|p| p.join("Steam").join("steamapps").join("common").join("MagicaVoxel").join("MagicaVoxel.exe")));
    places.into_iter().find(|p| p.exists())
}

impl Editor {
    /// Opens it over `world`, its floor high over where `eye` is, in voxels, or closes it
    pub fn toggle(&mut self, world: &World, eye: Vector3<f64>, game: &mut Game) {
        self.open = !self.open;
        if !self.open {
            game.set_preview(Vec::new());
            self.built = None;
            return;
        }
        if self.library.is_none() {
            self.library = Some(Library::load());
        }
        if self.entries.is_empty() {
            self.entries = entries(game, self.library.as_ref().unwrap());
            let n = assets::frames();
            let frames_of = |l: Look| -> Vec<&Model> { (0..n).map(|f| game.base_model(assets::look_model(l, f))).collect() };
            self.skins = Look::ALL.iter().map(|&l| assets::skin_of(l, &frames_of).map(|(from, _)| from)).collect();
        }
        // High over the ground round the camera, clear of anything in the way
        let vpm = world.voxels_per_meter as f64;
        let mut top = 0i64;
        for i in -5..=5 {
            for j in -5..=5 {
                let (x, y) = ((eye.x + i as f64 * 4.0 * vpm) as i64, (eye.y + j as f64 * 4.0 * vpm) as i64);
                if x < 0 || y < 0 || x >= world.size[0] as i64 || y >= world.size[1] as i64 {
                    continue;
                }
                if let Some(z) = (0..world.size[2] as i64).rev().find(|&z| !is_passable(world.material_at(x, y, z))) {
                    top = top.max(z);
                }
            }
        }
        let z = (top as f64 + 3.0 * vpm).min(world.size[2] as f64 - 24.0 * vpm).max(top as f64 + 1.0);
        self.stage = Vector3::new(eye.x, eye.y, z);
        // In the studio, on its floor in its middle
        if is_app() {
            self.stage = Vector3::new(world.size[0] as f64 * 0.5, world.size[1] as f64 * 0.5, 8.0);
        }
        self.focus_subject(game);
    }

    fn subject(&self) -> Option<Subject> {
        self.entries.get(self.pick).map(|e| e.subject)
    }

    fn rig(&self) -> Option<Box<dyn Rig>> {
        match self.subject()? {
            Subject::Dragon(look) => Some(Box::new(crate::dragons::EditorRig(look))),
            Subject::Boss(look) => Some(Box::new(crate::rig::edit::EditorRig(look))),
            Subject::Winged(look) => Some(Box::new(crate::avians::EditorRig(look))),
            Subject::Baked(look) => Some(Box::new(crate::baked::EditorRig(look))),
            _ => None,
        }
    }

    /// How far out a subject reaches, in meters, to see all of it
    fn reach(&self) -> f64 {
        match self.subject() {
            Some(Subject::Look(l)) | Some(Subject::Dragon(l)) | Some(Subject::Boss(l)) | Some(Subject::Winged(l)) | Some(Subject::Baked(l)) => look_reach(l),
            Some(Subject::Serpent(g)) => crate::serpents::GREATS[g].head * 2.0 + 2.0,
            Some(Subject::Wings(_)) => 1.0,
            Some(Subject::Weapon(_)) => 1.0,
            None => 3.0,
        }
    }

    /// The camera set back to see all of what's picked, its clip from the start
    fn focus_subject(&mut self, game: &mut Game) {
        let r = self.reach();
        self.distance = (r * 2.6).max(1.5);
        self.height = (r * 0.45).max(0.3);
        self.clip = 0;
        self.time = 0.0;
        self.piece = None;
        self.clip_edit = None;
        self.dirty = false;
        if let (Some(Subject::Weapon(f)), Some(library)) = (self.subject(), &self.library) {
            if self.parts.len() != library.families[f].slots.len() || self.built.as_ref().is_some_and(|b| b.0 != f) {
                self.parts = library.reference_parts(f);
            }
        } else {
            game.set_preview(Vec::new());
            self.built = None;
        }
        self.load_clip();
    }

    /// The clips of what's picked: a rig's, or a look's from its frames
    fn timelines(&self) -> Vec<Timeline> {
        match self.subject() {
            Some(Subject::Look(look)) => look_timelines(look),
            Some(Subject::Dragon(_)) | Some(Subject::Boss(_)) | Some(Subject::Winged(_)) => self.rig().map_or(Vec::new(), |r| r.clips().into_iter().map(|c| Timeline { name: c.name, length: c.length, frames: Vec::new() }).collect()),
            // Each clip with the frames baked from it, at their moments
            Some(Subject::Baked(look)) => {
                let rig = crate::baked::EditorRig(look);
                rig.clips().into_iter().enumerate().map(|(k, c)| Timeline { name: c.name, length: c.length, frames: rig.frames(k) }).collect()
            }
            Some(Subject::Weapon(_)) => {
                let n = self.built.as_ref().map_or(1, |_| self.preview_frames());
                vec![Timeline { name: "frames".into(), length: n as f64 * 0.5, frames: (0..n).map(|k| (k as f64 * 0.5, k)).collect() }]
            }
            _ => vec![Timeline { name: "still".into(), length: 1.0, frames: vec![(0.0, 0)] }],
        }
    }

    fn preview_frames(&self) -> usize {
        self.preview_count
    }

    /// The camera: where it looks at, in voxels, which way it faces and how far back it is, in voxels
    pub fn camera(&self, vpm: f64) -> (Vector3<f64>, f64, f64, f64) {
        (self.stage + Vector3::new(0.0, 0.0, self.height * vpm), self.yaw, self.pitch, self.distance * vpm)
    }

    /// Moves time on, and takes in what the mouse and keys did
    pub fn update(&mut self, dt: f64, ctl: &Controls, game: &mut Game) {
        if !self.over_ui {
            self.yaw -= ctl.drag.0 * 0.006;
            self.pitch = (self.pitch - ctl.drag.1 * 0.006).clamp(-FRAC_PI_2 + 0.05, FRAC_PI_2 - 0.05);
            if ctl.scroll != 0.0 {
                self.distance = (self.distance * (-ctl.scroll * 0.12).exp()).clamp(0.3, 120.0);
            }
        }
        let timelines = self.timelines();
        let length = timelines.get(self.clip).map_or(1.0, |t| t.length).max(1e-3);
        if self.playing {
            self.time += dt * self.speed;
            let looping = self.rig().and_then(|r| r.clips().get(self.clip).map(|c| c.looping)).unwrap_or(true);
            self.time = if looping || self.time <= length { self.time.rem_euclid(length + 1e-9) } else { 0.0 };
        }
        for &k in &ctl.keys {
            match k {
                ' ' => self.playing = !self.playing,
                'x' => self.axis = 0,
                'y' => self.axis = 1,
                'z' => self.axis = 2,
                'g' => self.moving = !self.moving,
                'q' => self.nudge(-1.0),
                'e' => self.nudge(1.0),
                'k' => self.key_now(None),
                'd' => self.unkey_now(),
                _ => {}
            }
        }
        if !self.over_ui && ctl.turn.0 != 0.0 {
            self.nudge(ctl.turn.0 * 0.1);
        }
        self.build_weapon(game);
        if let Some(e) = &self.exporting {
            let done = e.lock().unwrap_or_else(|e| e.into_inner()).take();
            if let Some(done) = done {
                self.note = done;
                self.exporting = None;
            }
        }
    }

    /// For screenshot runs, `--script editor`: what's picked and how it's shown at frame `frame`
    /// of the run, see App::run_script(); whether a shot's to be taken this frame
    pub fn scripted(&mut self, frame: usize, game: &mut Game) -> bool {
        let pick = |e: &mut Editor, title: &str, game: &mut Game| {
            if let Some(i) = e.entries.iter().position(|x| x.title == title) {
                e.pick = i;
                e.focus_subject(game);
            }
        };
        let bone = |e: &Editor, name: &str| e.rig().and_then(|r| r.bones().iter().position(|b| b.name == name));
        // `--set editor_hold=1` stays on the first, a shot every 150 frames, to see a file saved
        // meanwhile come in
        let hold = std::env::args().any(|a| a == "editor_hold=1");
        if hold && frame > 12 {
            return frame % 150 == 0;
        }
        match frame {
            12 => {
                pick(self, "Stone Warden", game);
                self.clip = self.timelines().iter().position(|t| t.name == "slam").unwrap_or(0);
                self.load_clip();
                self.time = 0.9;
                self.playing = false;
                self.yaw = 2.3;
            }
            72 => {
                pick(self, "Ember Drake", game);
                self.clip = 2;
                self.load_clip();
                self.time = 0.2;
                self.playing = false;
                self.piece = bone(self, "wing_left_outer");
                // A key on the bone picked, to see it turned on top of the code's pose
                self.key_now(Some(([0.0, 0.0, 35.0], [0.0; 3])));
                self.yaw = 2.6;
                self.pitch = -0.45;
            }
            132 => {
                pick(self, "Sword", game);
                self.first_person = true;
            }
            192 => {
                self.first_person = false;
                self.piece = self.library.as_ref().and_then(|l| match self.subject() {
                    Some(Subject::Weapon(f)) => l.families[f].slots.iter().position(|s| s == "blade"),
                    _ => None,
                });
                self.part_only = true;
                self.distance = 1.6;
                self.height = 0.7;
            }
            252 => {
                self.part_only = false;
                pick(self, "Gullfeather Wings", game);
                self.piece = Some(0);
            }
            // A monster baked from pieces, its blade arm picked and keyed raised further, see
            // src\baked.rs
            312 => {
                pick(self, "Armoured skeleton", game);
                self.clip = self.timelines().iter().position(|t| t.name == "windup").unwrap_or(0);
                self.load_clip();
                self.time = 1.0;
                self.playing = false;
                self.piece = bone(self, "arm_right");
                self.key_now(Some(([0.0, -40.0, 0.0], [0.0; 3])));
                self.yaw = 2.4;
                self.distance = 4.0;
            }
            _ => {}
        }
        matches!(frame, 70 | 130 | 190 | 250 | 310 | 370)
    }

    /// The bone picked turned or moved a step along the axis picked, keyed where the time is
    fn nudge(&mut self, steps: f64) {
        let (Some(rig), Some(b)) = (self.rig(), self.piece) else { return };
        let bones = rig.bones();
        let Some(bone) = bones.get(b) else { return };
        let (mut r, mut o) = self.clip_edit.as_ref().and_then(|c| c.sample(&bone.name, self.time)).unwrap_or(([0.0; 3], [0.0; 3]));
        if self.moving {
            o[self.axis] += 0.02 * steps;
        } else {
            r[self.axis] += 5.0 * steps;
        }
        self.key_now(Some((r, o)));
    }

    /// Keys the bone picked where the time is, as it is now or as given
    fn key_now(&mut self, set: Option<([f64; 3], [f64; 3])>) {
        let (Some(rig), Some(b)) = (self.rig(), self.piece) else { return };
        let bones = rig.bones();
        let Some(bone) = bones.get(b) else { return };
        let clip = self.clip_edit.get_or_insert_with(Clip::default);
        let (r, o) = set.or_else(|| clip.sample(&bone.name, self.time)).unwrap_or(([0.0; 3], [0.0; 3]));
        let ease = clip.tracks.get(&bone.name).and_then(|k| k.iter().find(|k| (k.time - self.time).abs() < 1e-4)).map_or(Ease::Smooth, |k| k.ease);
        clip.set_key(&bone.name, Key { time: (self.time * 1000.0).round() / 1000.0, rotate: r.map(|v| (v * 100.0).round() / 100.0), offset: o.map(|v| (v * 1000.0).round() / 1000.0), ease });
        self.dirty = true;
    }

    fn unkey_now(&mut self) {
        let (Some(rig), Some(b)) = (self.rig(), self.piece) else { return };
        let name = rig.bones()[b].name.clone();
        if let Some(c) = &mut self.clip_edit {
            let at = c.tracks.get(&name).and_then(|k| k.iter().min_by(|a, b| (a.time - self.time).abs().total_cmp(&(b.time - self.time).abs())).map(|k| k.time));
            if let Some(at) = at {
                c.remove_key(&name, at);
                self.dirty = true;
            }
        }
    }

    /// The clip picked as its file has it, to change, or a new one
    fn load_clip(&mut self) {
        let Some(rig) = self.rig() else {
            self.clip_edit = None;
            return;
        };
        let clips = rig.clips();
        let Some(def) = clips.get(self.clip) else { return };
        anim::forget();
        let loaded = anim::load(&rig.id(), &def.name).map(|c| (*c).clone());
        self.for_kind = loaded.as_ref().is_some_and(|c| c.rig != rig.id() && !c.rig.is_empty());
        let mut clip = loaded.unwrap_or_default();
        clip.clip = def.name.clone();
        clip.length = def.length;
        clip.looping = def.looping;
        self.clip_edit = Some(clip);
        self.dirty = false;
    }

    fn save_clip(&mut self) {
        let (Some(rig), Some(clip)) = (self.rig(), &self.clip_edit) else { return };
        let id = rig.id();
        let id = if self.for_kind { id.split('/').next().unwrap_or(&id).to_string() } else { id };
        match anim::save(&id, clip) {
            Ok(path) => {
                self.note = format!("Saved {}", path.display());
                self.dirty = false;
            }
            Err(e) => self.note = format!("Couldn't save: {e}"),
        }
    }

    /// The weapon picked made of the parts picked, or the part picked alone, for drawing
    fn build_weapon(&mut self, game: &mut Game) {
        let (Some(Subject::Weapon(f)), Some(library)) = (self.subject(), &self.library) else { return };
        let want = (f, self.parts.clone(), self.part_only, self.piece);
        if self.built.as_ref() == Some(&want) {
            return;
        }
        let look = weapon_parts::build(library, f, &self.parts, || crate::weapons::look_of(library, f, &self.parts));
        let mut frames: Vec<(Model, Vector3<f64>)> = look.frames.iter().map(|(m, a)| (assets::copy(m), *a)).collect();
        if self.part_only
            && let Some(s) = self.piece
        {
            // The part alone, from its file if it has one, where it is on the weapon
            let (code, tags) = weapon_parts::tagged_look(library, f, &self.parts);
            frames = code
                .frames
                .iter()
                .enumerate()
                .filter_map(|(j, (m, a))| {
                    let (part, lo) = weapon_parts::part_of(m, &tags[j], s as u8 + 1)?;
                    Some((assets::copy(&part), assets::moved_anchor(*a, m.size, lo, part.size, code.res)))
                })
                .collect();
            if let Some(name) = self.piece_file()
                && let Some(first) = frames.first_mut()
            {
                let path = assets::file_of(&assets::root(), &name);
                if let Ok(edit) = assets::read_edit(&path, [0.0; 3])
                    && path.with_extension("json").exists()
                {
                    let (m, lo) = assets::conform(&edit, first.0.size, assets::Grow::Free);
                    first.1 = assets::moved_anchor(first.1, first.0.size, lo, m.size, look.res);
                    first.0 = m;
                }
            }
        }
        self.preview_res = look.res;
        self.preview_count = frames.len().max(1);
        game.set_preview(frames);
        self.built = Some(want);
    }

    /// The look `look` is drawn from in its own colours, if it shares another's shape
    fn skin_source(&self, look: Look) -> Option<Look> {
        Look::ALL.iter().position(|&l| l == look).and_then(|i| self.skins.get(i).copied().flatten())
    }

    /// The file of the piece picked, as assets\models has it, without .vox
    fn piece_file(&self) -> Option<String> {
        let library = self.library.as_ref()?;
        Some(match self.subject()? {
            Subject::Look(look) => {
                let from = self.skin_source(look).unwrap_or(look);
                format!("{}/base", assets::look_folder(from))
            }
            Subject::Dragon(look) => {
                let b = self.piece.unwrap_or(0);
                format!("dragon/{}/{}", assets::snake(&format!("{look:?}")), assets::dragon_piece_name(dragon_piece(b)))
            }
            Subject::Boss(look) => {
                let k = crate::rig::kind(look)?;
                assets::boss_file(k * crate::rig::PIECES + self.piece.unwrap_or(0))?
            }
            // From the look it shares its shape with, if it does
            Subject::Baked(look) => {
                let from = crate::baked::skin_of(look).map_or(look, |(from, _)| from);
                format!("{}/{}", assets::look_folder(from), crate::baked::piece_name(from, self.piece.unwrap_or(0))?)
            }
            Subject::Winged(look) => {
                let k = crate::avians::KINDS.iter().position(|&l| l == look)?;
                assets::avian_file(k * assets::avian_pieces() + winged_piece(self.piece.unwrap_or(0)))?
            }
            Subject::Serpent(g) => format!("serpent/{}/{}", assets::snake(&format!("{:?}", crate::serpents::GREATS[g].look)), ["head", "jaw", "body"][self.piece.unwrap_or(0).min(2)]),
            Subject::Wings(k) => format!("wings/{}", crate::wings::KINDS[k].id),
            Subject::Weapon(f) => {
                let s = self.piece.unwrap_or(0);
                weapon_parts::part_file(&library.families[f].id, &weapon_parts::part_name(library, *self.parts.get(s)?), 0)
            }
        })
    }

    /// Writes the piece picked's file, as the game draws it now, if it isn't there yet
    fn export_piece(&mut self, game: &Game) -> Result<PathBuf, String> {
        let name = self.piece_file().ok_or("nothing picked")?;
        let dir = assets::root();
        let path = assets::file_of(&dir, &name);
        if path.exists() {
            return Ok(path);
        }
        let library = self.library.as_ref().ok_or("no weapons")?;
        match self.subject().ok_or("nothing picked")? {
            Subject::Look(look) => {
                let from = self.skin_source(look).unwrap_or(look);
                let m = game.base_model(assets::look_model(from, 0));
                assets::write_model(&dir, &name, m, assets::look_res(from), None, &format!("{}, standing frame", assets::look_title(from)), assets::LOOK_NOTE)?;
            }
            Subject::Dragon(look) => {
                let k = crate::dragons::KINDS.iter().position(|&l| l == look).unwrap_or(0);
                let i = k * assets::dragon_pieces() + dragon_piece(self.piece.unwrap_or(0));
                let m = game.base_model(animals::DRAGON_MODELS_AT + i);
                assets::write_model(&dir, &name, m, MODEL_RES, crate::dragons::anchors().get(i).copied(), &name, "A jointed piece: the pivot is its joint.")?;
            }
            Subject::Boss(look) => {
                let i = crate::rig::kind(look).unwrap_or(0) * crate::rig::PIECES + self.piece.unwrap_or(0);
                let m = game.base_model(animals::RIG_MODELS_AT + i);
                assets::write_model(&dir, &name, m, MODEL_RES, crate::rig::edit::anchors().get(i).copied(), &name, "A jointed piece: the pivot is its joint.")?;
            }
            Subject::Baked(look) => {
                let from = crate::baked::skin_of(look).map_or(look, |(from, _)| from);
                let rig = crate::baked::rig(from).ok_or("not baked from pieces")?;
                let piece = rig.pieces.get(self.piece.unwrap_or(0)).filter(|p| !p.is_empty()).ok_or("no such piece")?;
                assets::write_model(&dir, &name, &piece.model, MODEL_RES, Some(piece.anchor()), &name, assets::PIECE_NOTE)?;
            }
            Subject::Winged(look) => {
                let i = crate::avians::KINDS.iter().position(|&l| l == look).unwrap_or(0) * assets::avian_pieces() + winged_piece(self.piece.unwrap_or(0));
                let m = game.base_model(animals::WINGED_MODELS_AT + i);
                assets::write_model(&dir, &name, m, MODEL_RES, crate::avians::anchors().get(i).copied(), &name, "A jointed piece: the pivot is its joint.")?;
            }
            Subject::Serpent(g) => {
                let i = g * 3 + self.piece.unwrap_or(0).min(2);
                let m = game.base_model(animals::ANIMAL_MODELS - crate::serpents::PART_MODELS + i);
                assets::write_model(&dir, &name, m, MODEL_RES, crate::serpents::anchors().get(i).copied(), &name, "")?;
            }
            Subject::Wings(k) => {
                let (m, a) = &game.wing_models()[k];
                assets::write_model(&dir, &name, m, assets::WING_RES, Some(*a), crate::wings::KINDS[k].name, "The right wing; the left is it mirrored.")?;
            }
            Subject::Weapon(f) => {
                let s = self.piece.unwrap_or(0);
                weapon_parts::export_part(&dir, library, f, s, self.parts[s])?;
            }
        }
        Ok(path)
    }

    fn open_in_magicavoxel(&mut self, game: &Game) {
        match self.export_piece(game) {
            Ok(path) => match magicavoxel() {
                Some(exe) => {
                    let started = std::process::Command::new(&exe).arg(&path).current_dir(exe.parent().unwrap_or(Path::new("."))).spawn();
                    self.note = match started {
                        Ok(_) => format!("Opened {} in MagicaVoxel. Save it there and it's here at once.", path.display()),
                        Err(e) => format!("Couldn't start {}: {e}. The file's {}", exe.display(), path.display()),
                    };
                }
                None => self.note = format!("MagicaVoxel wasn't found (set it with --set magicavoxel=<path>). The file's {}", path.display()),
            },
            Err(e) => self.note = format!("Couldn't write the file: {e}"),
        }
    }

    /// What to draw: the floor, and what's picked on it as it is now
    pub fn instances(&self, game: &Game, vpm: f64, eye: Vector3<f64>, look_yaw: f64, look_pitch: f64) -> Vec<AnimalInstance> {
        let mut out = Vec::new();
        let floor = FLOOR_SIDE as f64 * FLOOR_CELL;
        if !is_app() {
            out.push(AnimalInstance::new(game.stage_model(), self.stage - Vector3::new(0.0, 0.0, FLOOR_CELL * vpm), UnitQuaternion::identity(), FLOOR_CELL * vpm, floor * 0.75 * vpm, STILL));
        }
        let at = self.stage;
        let lit = |i: AnimalInstance, on: bool| if on { i.skinned(0xA0_30_D0_FF) } else { i };
        let timelines = self.timelines();
        match self.subject() {
            Some(Subject::Look(look)) => {
                let frame = timelines.get(self.clip).map_or(0, |t| t.frame_at(self.time));
                let res = assets::look_res(look);
                let i = AnimalInstance::new(assets::look_model(look, frame) as u32, at, UnitQuaternion::identity(), vpm / res, look.reach() * vpm, MOVING);
                out.push(lit(i, self.piece.is_some()));
            }
            Some(Subject::Dragon(_)) | Some(Subject::Winged(_)) => {
                if let Some(rig) = self.rig() {
                    let joints = self.posed(rig.as_ref());
                    // Its middle over the floor as it stands
                    let feet = joints.iter().map(|j| j.at.z).fold(0.0f64, f64::min);
                    rig.draw(&joints, at + Vector3::new(0.0, 0.0, (-feet).max(0.0) * vpm + 0.6 * vpm), 0.0, vpm, self.piece, &mut out);
                }
            }
            Some(Subject::Boss(_)) => {
                // Its joints are from the middle of its feet
                if let Some(rig) = self.rig() {
                    let joints = self.posed(rig.as_ref());
                    rig.draw(&joints, at, 0.0, vpm, self.piece, &mut out);
                }
            }
            // While a bone's picked or its clip's being changed, its pieces where they go now, as
            // a jointed boss's are; otherwise the frame baked from them, as the game draws it, baked
            // again from the clip once it's saved
            Some(Subject::Baked(look)) => {
                if self.dirty || self.piece.is_some() {
                    if let Some(rig) = self.rig() {
                        let joints = self.posed(rig.as_ref());
                        rig.draw(&joints, at, 0.0, vpm, self.piece, &mut out);
                    }
                } else {
                    let frame = timelines.get(self.clip).map_or(0, |t| t.frame_at(self.time));
                    out.push(AnimalInstance::new(assets::look_model(look, frame) as u32, at, UnitQuaternion::identity(), vpm / MODEL_RES, look.reach() * vpm, MOVING));
                }
            }
            Some(Subject::Serpent(g)) => {
                let anchors = crate::serpents::anchors();
                let first = animals::ANIMAL_MODELS - crate::serpents::PART_MODELS;
                let great = &crate::serpents::GREATS[g];
                for p in 0..3 {
                    let i = g * 3 + p;
                    let m = game.base_model(first + i);
                    let place = at + Vector3::new(0.0, (p as f64 - 1.0) * great.head * 1.6 * vpm, great.girth * vpm);
                    let size = m.size.map(|s| s as f64 / MODEL_RES);
                    let reach = (size[0] * size[0] + size[1] * size[1] + size[2] * size[2]).sqrt() * vpm;
                    let inst = AnimalInstance::new((first + i) as u32, place + anchors[i] * vpm, UnitQuaternion::identity(), vpm / MODEL_RES, reach, MOVING);
                    out.push(lit(inst, self.piece == Some(p)));
                }
            }
            Some(Subject::Wings(k)) => {
                let (m, a) = &game.wing_models()[k];
                let res = assets::WING_RES;
                let size = m.size.map(|s| s as f64 / res);
                let reach = (size[0] * size[0] + size[1] * size[1] + size[2] * size[2]).sqrt() * vpm;
                // Stood up on its leading edge, so it's seen whole
                let turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), FRAC_PI_2);
                let place = at + Vector3::new(0.0, 0.0, 0.2 * vpm);
                let i = AnimalInstance::new(game.wing_model(k), place + turn * a * vpm, turn, vpm / res, reach, MOVING);
                out.push(lit(i, self.piece.is_some()));
            }
            Some(Subject::Weapon(_)) => {
                let k = timelines.first().map_or(0, |t| t.frame_at(self.time)).min(self.preview_count.saturating_sub(1));
                if let Some((model, anchor, size)) = game.preview_model(k) {
                    let s = size.map(|v| v as f64 / self.preview_res);
                    let reach = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt() * vpm;
                    if self.first_person {
                        // Held in front of the eye as the player sees what's in their hand: low
                        // on the right, leaning forward and in
                        let view = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), look_yaw) * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), look_pitch);
                        let (right, forward, up) = (view * Vector3::x(), view * Vector3::y(), view * Vector3::z());
                        let hand = eye + (right * 0.2 + forward * 0.45 - up * 0.32) * vpm;
                        let turn = view * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.55) * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), -0.35);
                        out.push(AnimalInstance::new(model, hand + turn * anchor * vpm, turn, vpm / self.preview_res, reach, HELD));
                    } else {
                        let base = at + Vector3::new(0.0, 0.0, 0.3 * vpm);
                        out.push(AnimalInstance::new(model, base + anchor * vpm, UnitQuaternion::identity(), vpm / self.preview_res, reach, MOVING));
                    }
                }
            }
            None => {}
        }
        out
    }

    /// A rig's joints as the clip picked has them now: the code's pose with the clip's keys on top
    fn posed(&self, rig: &dyn Rig) -> Vec<Joint> {
        let mut joints = rig.pose(self.clip, self.time);
        if let Some(c) = &self.clip_edit {
            let bones = rig.bones();
            anim::apply_deltas(&bones, &mut joints, &|b| c.sample(&bones[b].name, self.time), 1.0);
        }
        joints
    }

    /// The editor's windows: what there is to pick on the left, what's picked's pieces and bones
    /// and how to change them on the right, its animations along the bottom
    pub fn ui(&mut self, ctx: &egui::Context, game: &mut Game) -> bool {
        let mut reload = false;
        let mut picked = None;
        let frame = egui::Frame::new().fill(Color32::from_black_alpha(200)).inner_margin(8.0);
        egui::SidePanel::left("editor list").frame(frame).default_width(230.0).show(ctx, |ui| {
            ui.heading("Model editor");
            ui.label(RichText::new("Drag to turn · scroll to zoom").small());
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search"));
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut group = "";
                let search = self.search.to_lowercase();
                for (i, e) in self.entries.iter().enumerate() {
                    if !search.is_empty() && !e.title.to_lowercase().contains(&search) {
                        continue;
                    }
                    if e.group != group {
                        group = e.group;
                        ui.add_space(4.0);
                        ui.label(RichText::new(group).strong().color(Color32::from_rgb(230, 200, 120)));
                    }
                    if ui.selectable_label(i == self.pick, &e.title).clicked() {
                        picked = Some(i);
                    }
                }
            });
        });
        if let Some(i) = picked
            && i != self.pick
        {
            self.pick = i;
            self.focus_subject(game);
        }

        egui::SidePanel::right("editor pieces").frame(frame).default_width(270.0).show(ctx, |ui| {
            self.pieces_ui(ui, game);
            ui.separator();
            if ui.button("Read every model file again").clicked() {
                reload = true;
            }
            let busy = self.exporting.is_some();
            if ui.add_enabled(!busy, egui::Button::new(if busy { "Exporting..." } else { "Export every model" })).clicked() {
                let done = Arc::new(Mutex::new(None));
                let report = done.clone();
                std::thread::spawn(move || {
                    let r = assets::export_all(&assets::root(), false);
                    *report.lock().unwrap_or_else(|e| e.into_inner()) = Some(match r {
                        Ok(e) => format!("Wrote {} models to {}", e.lines.len(), assets::root().display()),
                        Err(e) => format!("Couldn't export: {e}"),
                    });
                });
                self.exporting = Some(done);
            }
            if !self.note.is_empty() {
                ui.label(RichText::new(&self.note).small().color(Color32::from_rgb(200, 220, 255)));
            }
            for line in assets::recent_log().iter().rev().take(4) {
                ui.label(RichText::new(line).small().color(Color32::from_gray(170)));
            }
        });

        egui::TopBottomPanel::bottom("editor timeline").frame(frame).show(ctx, |ui| self.timeline_ui(ui));
        self.over_ui = ctx.is_pointer_over_area() || ctx.wants_pointer_input();
        reload
    }

    fn pieces_ui(&mut self, ui: &mut egui::Ui, game: &mut Game) {
        let Some(subject) = self.subject() else { return };
        let title = self.entries[self.pick].title.clone();
        ui.heading(title);
        let mut open = false;
        match subject {
            Subject::Look(look) => {
                if let Some(from) = self.skin_source(look) {
                    ui.label(format!("Shares its shape with {}, in its own colours.", assets::look_title(from)));
                }
                ui.label("Pieces");
                if ui.selectable_label(self.piece == Some(0), "base: the standing frame").clicked() {
                    self.piece = if self.piece == Some(0) { None } else { Some(0) };
                }
                ui.label(RichText::new("Baked frames: changes to the standing frame are made in every frame.").small());
            }
            Subject::Dragon(_) | Subject::Boss(_) | Subject::Winged(_) | Subject::Baked(_) => {
                if let Subject::Baked(look) = subject {
                    if let Some((from, _)) = crate::baked::skin_of(look) {
                        ui.label(format!("Shares its pieces with {}, in its own colours.", assets::look_title(from)));
                    }
                    ui.label(RichText::new("Its frames are baked from these pieces: a piece changed goes with it in every frame. Pick none to see the frames as the game draws them.").small());
                }
                if let Some(rig) = self.rig() {
                    ui.label("Bones");
                    egui::ScrollArea::vertical().id_salt("bones").max_height(260.0).show(ui, |ui| {
                        for (b, bone) in rig.bones().iter().enumerate() {
                            let keyed = self.clip_edit.as_ref().is_some_and(|c| c.tracks.contains_key(&bone.name));
                            let text = if keyed { format!("{} *", bone.name) } else { bone.name.clone() };
                            if ui.selectable_label(self.piece == Some(b), text).clicked() {
                                self.piece = Some(b);
                            }
                        }
                    });
                    self.bone_ui(ui, rig.as_ref());
                }
            }
            Subject::Serpent(_) => {
                ui.label("Pieces");
                for (p, name) in ["head", "jaw", "body"].iter().enumerate() {
                    if ui.selectable_label(self.piece == Some(p), *name).clicked() {
                        self.piece = Some(p);
                    }
                }
            }
            Subject::Wings(_) => {
                if ui.selectable_label(self.piece == Some(0), "the wing (the left's it mirrored)").clicked() {
                    self.piece = Some(0);
                }
            }
            Subject::Weapon(f) => {
                if let Some(library) = &self.library {
                    ui.label("Parts");
                    for (s, (slot, fits)) in weapon_parts::slots_of(library, f).into_iter().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.selectable_label(self.piece == Some(s), &slot).clicked() {
                                self.piece = Some(s);
                            }
                            let current = self.parts.get(s).copied().unwrap_or(0);
                            egui::ComboBox::from_id_salt(("part", s)).selected_text(library.components[current].display_name.clone()).width(150.0).show_ui(ui, |ui| {
                                for c in fits {
                                    if ui.selectable_label(c == current, &library.components[c].display_name).clicked() && s < self.parts.len() {
                                        self.parts[s] = c;
                                    }
                                }
                            });
                        });
                    }
                    ui.checkbox(&mut self.part_only, "Show only the part picked");
                    ui.checkbox(&mut self.first_person, "Held in first person");
                    if ui.button("Reference weapon").clicked() {
                        self.parts = library.reference_parts(f);
                    }
                }
            }
        }
        ui.add_space(6.0);
        if let Some(file) = self.piece_file() {
            let path = assets::file_of(&assets::root(), &file);
            ui.label(RichText::new(format!("{}{}", path.display(), if path.exists() { "" } else { " (not written yet)" })).small());
            if ui.button("Open in MagicaVoxel").clicked() {
                open = true;
            }
        }
        if open {
            self.open_in_magicavoxel(game);
        }
    }

    /// Changing the bone picked, for a rig
    fn bone_ui(&mut self, ui: &mut egui::Ui, rig: &dyn Rig) {
        let Some(b) = self.piece else {
            ui.label(RichText::new("Pick a bone to animate it.").small());
            return;
        };
        let bones = rig.bones();
        let name = bones[b].name.clone();
        ui.separator();
        ui.label(RichText::new(format!("{name} at {:.2} s", self.time)).strong());
        let (mut r, mut o) = self.clip_edit.as_ref().and_then(|c| c.sample(&name, self.time)).unwrap_or(([0.0; 3], [0.0; 3]));
        let mut changed = false;
        for (a, axis) in ["x", "y", "z"].iter().enumerate() {
            changed |= ui.add(egui::Slider::new(&mut r[a], -180.0..=180.0).text(format!("turn {axis}°"))).changed();
        }
        for (a, axis) in ["x", "y", "z"].iter().enumerate() {
            changed |= ui.add(egui::Slider::new(&mut o[a], -2.0..=2.0).text(format!("move {axis} m"))).changed();
        }
        if changed {
            self.key_now(Some((r, o)));
        }
        let key_here = self.clip_edit.as_ref().and_then(|c| c.tracks.get(&name)).and_then(|k| k.iter().find(|k| (k.time - self.time).abs() < 1e-3)).copied();
        ui.horizontal(|ui| {
            if ui.button("Key (K)").clicked() {
                self.key_now(None);
            }
            if ui.add_enabled(key_here.is_some(), egui::Button::new("Unkey")).clicked() {
                self.unkey_now();
            }
            if let Some(k) = key_here {
                let mut ease = k.ease;
                egui::ComboBox::from_id_salt("ease").selected_text(format!("{ease:?}")).show_ui(ui, |ui| {
                    for e in Ease::ALL {
                        ui.selectable_value(&mut ease, e, format!("{e:?}"));
                    }
                });
                if ease != k.ease
                    && let Some(c) = &mut self.clip_edit
                {
                    c.set_key(&name, Key { ease, ..k });
                    self.dirty = true;
                }
            }
        });
        ui.label(
            RichText::new(format!(
                "{} about {} · Q/E or right-drag nudge · X Y Z axis · G turn/move · K key · D unkey · Space play",
                if self.moving { "Moving" } else { "Turning" },
                ["x", "y", "z"][self.axis]
            ))
            .small(),
        );
        ui.horizontal(|ui| {
            if ui.add_enabled(self.dirty, egui::Button::new("Save")).clicked() {
                self.save_clip();
            }
            if ui.button("Revert").clicked() {
                self.load_clip();
            }
            ui.checkbox(&mut self.for_kind, "for all of its kind");
        });
    }

    fn timeline_ui(&mut self, ui: &mut egui::Ui) {
        let timelines = self.timelines();
        if timelines.is_empty() {
            return;
        }
        let mut clip = self.clip.min(timelines.len() - 1);
        ui.horizontal(|ui| {
            for (i, t) in timelines.iter().enumerate() {
                if ui.selectable_label(i == clip, &t.name).clicked() {
                    clip = i;
                }
            }
        });
        if clip != self.clip {
            if self.dirty {
                self.note = "The clip changed had unsaved keys; they're dropped.".into();
            }
            self.clip = clip;
            self.time = 0.0;
            self.load_clip();
        }
        let length = timelines[clip].length.max(1e-3);
        ui.horizontal(|ui| {
            if ui.button(if self.playing { "Pause" } else { "Play" }).clicked() {
                self.playing = !self.playing;
            }
            ui.add(egui::Slider::new(&mut self.speed, 0.05..=1.0).text("speed"));
            ui.label(format!("{:.2} / {:.2} s", self.time, length));
        });
        // The scrub bar, with the keys of the bone picked marked on it
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, Color32::from_gray(40));
        let x_of = |t: f64| rect.left() + (t / length) as f32 * rect.width();
        if let (Some(rig), Some(b), Some(c)) = (self.rig(), self.piece, &self.clip_edit)
            && let Some(keys) = rig.bones().get(b).and_then(|bone| c.tracks.get(&bone.name))
        {
            for k in keys {
                painter.circle_filled(egui::pos2(x_of(k.time), rect.center().y), 5.0, Color32::from_rgb(250, 200, 80));
            }
        }
        for (t, _) in &timelines[clip].frames {
            painter.line_segment([egui::pos2(x_of(*t), rect.bottom() - 6.0), egui::pos2(x_of(*t), rect.bottom())], egui::Stroke::new(1.0_f32, Color32::from_gray(120)));
        }
        painter.line_segment([egui::pos2(x_of(self.time), rect.top()), egui::pos2(x_of(self.time), rect.bottom())], egui::Stroke::new(2.0_f32, Color32::WHITE));
        if (response.dragged() || response.clicked())
            && let Some(p) = response.interact_pointer_pos()
        {
            self.time = (((p.x - rect.left()) / rect.width()) as f64 * length).clamp(0.0, length);
            self.playing = false;
        }
    }
}

/// Everything there is to show, grouped
fn entries(game: &Game, library: &Library) -> Vec<Entry> {
    let mut out = Vec::new();
    let group_of = |l: Look| -> &'static str {
        if l.is_raid() || l.is_boss() {
            "Bosses"
        } else if l.is_monster() {
            "Monsters"
        } else if l.is_shrine() || matches!(l, Look::Apple | Look::Meat | Look::Candle | Look::Skull | Look::Chest | Look::OpenChest | Look::Coins | Look::Potion | Look::Bread | Look::ShedSkin | Look::SerpentBones | Look::SerpentTongue) {
            "Things"
        } else {
            "Animals and people"
        }
    };
    let mut looks: Vec<Entry> = Look::ALL
        .iter()
        .filter(|l| !l.is_dragon() && !l.is_great_serpent() && !crate::rig::rigged(**l) && !crate::avians::is_winged(**l))
        .filter(|&&l| {
            let m = game.base_model(assets::look_model(l, 0));
            m.size != [1, 1, 1]
        })
        .map(|&l| Entry { subject: if crate::baked::baked(l) { Subject::Baked(l) } else { Subject::Look(l) }, title: assets::look_title(l), group: group_of(l) })
        .collect();
    let order = ["Bosses", "Monsters", "Animals and people", "Things"];
    looks.sort_by_key(|e| order.iter().position(|g| *g == e.group));
    out.extend(looks);
    out.extend(crate::rig::KINDS.iter().map(|&l| Entry { subject: Subject::Boss(l), title: assets::look_title(l), group: "Jointed bosses" }));
    out.extend(crate::dragons::KINDS.iter().map(|&l| Entry { subject: Subject::Dragon(l), title: assets::look_title(l), group: "Dragons" }));
    out.extend(crate::avians::KINDS.iter().map(|&l| Entry { subject: Subject::Winged(l), title: assets::look_title(l), group: "Avians and gargoyles" }));
    out.extend(crate::serpents::GREATS.iter().enumerate().map(|(g, s)| Entry { subject: Subject::Serpent(g), title: assets::look_title(s.look), group: "Great serpents" }));
    out.extend(crate::wings::KINDS.iter().enumerate().map(|(k, w)| Entry { subject: Subject::Wings(k), title: w.name.to_string(), group: "Wings" }));
    out.extend(library.families.iter().enumerate().map(|(f, fam)| Entry { subject: Subject::Weapon(f), title: fam.display_name.clone(), group: if fam.tool.is_some() { "Tools" } else { "Weapons" } }));
    out
}

/// A look's animations, from its frames: standing, walking, and each of its attacks from standing
/// back to standing, as the game goes between them, see animals::tween()
fn look_timelines(look: Look) -> Vec<Timeline> {
    let mut out = vec![Timeline { name: "idle".into(), length: 1.0, frames: vec![(0.0, 0)] }];
    let walk = animals::POSE_WINDUP - 1;
    if look_walks(look) {
        out.push(Timeline { name: "walk".into(), length: 1.0, frames: (0..walk).map(|k| (k as f64 / walk as f64, k + 1)).collect() });
    }
    if look.build().is_none() {
        return out;
    }
    use animals::{POSE_COIL, POSE_RAISE, POSE_SPECIAL, POSE_SPIKES, POSE_STRIKE, POSE_SWEEP, POSE_WINDUP};
    let mut attacks: Vec<(&str, Vec<(usize, usize)>)> = vec![
        ("windup", vec![(0, POSE_WINDUP), (POSE_WINDUP, 0)]),
        ("strike", vec![(0, POSE_WINDUP), (POSE_WINDUP, POSE_STRIKE), (POSE_STRIKE, 0)]),
        ("special", vec![(0, POSE_SPECIAL), (POSE_SPECIAL, POSE_STRIKE), (POSE_STRIKE, 0)]),
    ];
    if look.is_knight() {
        attacks.push(("sweep", vec![(0, POSE_COIL), (POSE_COIL, POSE_SWEEP), (POSE_SWEEP, 0)]));
        attacks.push(("spikes", vec![(0, POSE_RAISE), (POSE_RAISE, POSE_SPIKES), (POSE_SPIKES, 0)]));
    }
    // A serpent settling into its coils to rest, and out of them
    if look.is_serpent() {
        attacks.push(("coil", vec![(0, POSE_COIL), (POSE_COIL, 0)]));
    }
    for (name, steps) in attacks {
        let mut frames = Vec::new();
        let mut t = 0.0;
        for (a, b) in steps {
            let mut since = 0.0;
            while let Some(f) = animals::tween(look, a, b, since) {
                if frames.last().map(|(_, g)| *g) != Some(f) {
                    frames.push((t + since, f));
                }
                since += 0.01;
                if since > 3.0 {
                    break;
                }
            }
            t += since;
            frames.push((t, b));
            // Held there a moment, so it's seen
            t += 0.3;
        }
        out.push(Timeline { name: name.into(), length: t, frames });
    }
    out
}

/// Whether a look has walking frames
fn look_walks(look: Look) -> bool {
    !matches!(look, Look::Apple | Look::Meat | Look::Candle | Look::Skull | Look::Chest | Look::OpenChest | Look::Coins | Look::Potion | Look::Bread | Look::Merchant | Look::Princess | Look::RoyalGuard | Look::ShedSkin | Look::SerpentBones | Look::SerpentTongue)
        && !look.is_shrine()
        && !look.is_merchant()
        && look != Look::TradeStall
}
