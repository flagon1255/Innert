//! The weapon in hand drawn finer than every other, see weapons::HELD_FINE: right before the eye,
//! its voxels are the ones seen biggest. Only the one is made finer, as every weapon there is at
//! that grid would take about eight times the memory and the time to make. It's made on a thread
//! of its own, the same weapon at its usual grid drawn till it's done, and sent to the graphics card
//! after the weapons' models, each frame packed alone, so the rest aren't packed again for it, see
//! Game::weapon_model_words().

use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use crate::animals::pack_models;
use crate::weapons::{HELD_FINE, Library, Weapon, WeaponLook};

use super::Game;

/// Which weapon a fine look is of: its family, its parts, its seed and how big it's drawn, and how
/// many times the weapons' looks had been made again from their files, see set_weapon_looks()
type Key = (usize, Vec<usize>, u32, u64, u32);

/// A fine look being asked for, and one made, with each of its frames packed alone
type Job = (Key, f64);
type Made = (Key, WeaponLook, Vec<Vec<u32>>);

#[derive(Default)]
pub(super) struct Fine {
    /// What's drawn finer now, and its look and packed frames
    key: Option<Key>,
    look: Option<WeaponLook>,
    pub(super) packed: Vec<Vec<u32>>,
    /// What was last asked of the thread making them
    asked: Option<Key>,
    jobs: Option<Sender<Job>>,
    made: Option<Receiver<Made>>,
    /// How many times the weapons' looks have been made again from their files
    pub(super) version: u32,
}

/// The thread making fine looks: each the newest asked for, those asked for since skipped, with a
/// library of its own
fn worker(jobs: Receiver<Job>, made: Sender<Made>) {
    let library = Library::load();
    while let Ok(mut job) = jobs.recv() {
        while let Ok(newer) = jobs.try_recv() {
            job = newer;
        }
        let ((family, parts, seed, scale, version), k) = job;
        if family >= library.families.len() {
            continue;
        }
        let look = crate::weapons::fine_look(&library, family, &parts, f64::from_bits(scale), k);
        let packed = look.frames.iter().map(|(m, _)| pack_models(&[m])).collect();
        if made.send(((family, parts, seed, scale, version), look, packed)).is_err() {
            break;
        }
    }
}

fn key_of(w: &Weapon, version: u32) -> Key {
    (w.family, w.parts.clone(), w.seed, w.look.scale.to_bits(), version)
}

impl Game {
    /// Keeps the weapon in hand's fine look: asks for it to be made when another's taken in hand,
    /// and puts it in once it's made, to be sent with the weapons' models. Once a frame.
    pub fn fine_held(&mut self) {
        if let Some(made) = &self.fine.made {
            match made.try_recv() {
                Ok((key, look, packed)) => {
                    self.fine.key = Some(key);
                    self.fine.look = Some(look);
                    self.fine.packed = packed;
                    // Sent with the one-off models, the rest as they were, see weapon_model_words()
                    self.models_version += 1;
                    self.one_off_changes += 1;
                }
                Err(TryRecvError::Disconnected) => {
                    self.fine.made = None;
                    self.fine.jobs = None;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        if HELD_FINE <= 1.0 {
            return;
        }
        let Some((_, w)) = self.held() else { return };
        let want = key_of(w, self.fine.version);
        if self.fine.key.as_ref() == Some(&want) || self.fine.asked.as_ref() == Some(&want) {
            return;
        }
        if self.fine.jobs.is_none() {
            let (jobs, rx) = channel();
            let (tx, made) = channel();
            if std::thread::Builder::new().name("fine weapon".into()).spawn(move || worker(rx, tx)).is_err() {
                return;
            }
            self.fine.jobs = Some(jobs);
            self.fine.made = Some(made);
        }
        if let Some(jobs) = &self.fine.jobs
            && jobs.send((want.clone(), HELD_FINE)).is_ok()
        {
            self.fine.asked = Some(want);
        }
    }

    /// The look the weapon `w` in hand is drawn with and where its models start among all models:
    /// its fine look once that's made, otherwise its own at `model`
    pub(super) fn held_look<'a>(&'a self, w: &'a Weapon, model: u32) -> (&'a WeaponLook, u32) {
        match &self.fine.look {
            Some(look) if self.fine.key == Some(key_of(w, self.fine.version)) && look.frames.len() == w.look.frames.len() => (look, self.fine_model()),
            _ => (&w.look, model),
        }
    }

    /// Where the fine look's first model is among all models: after the one-off models
    fn fine_model(&self) -> u32 {
        self.weapon_model(self.weapons.len()) + (self.preview.len() + self.body_models.len() + self.one_offs.len()) as u32
    }
}
