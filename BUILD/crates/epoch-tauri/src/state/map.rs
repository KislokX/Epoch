//! The World's geography, and undoing a change to it (ADR-0028).
//!
//! Split out of `state.rs` unchanged: Places, land, artwork, and the edit history that
//! makes a wrong drag survivable. The freeze lives here rather than beside the turn it
//! protects, because it is the map it exists to hold still.

use super::*;

/// Why a turn is being prepared.
///
/// `said: Option<&str>` used to carry this: `Some` meant the user typed, `None` meant a handover.
/// Both are real turns, so both mark the character as working — which is correct, and it is why
/// re-using the same call merely to *read* the composed Report left somebody thinking with a
/// model that was not running. They stayed "WORKING" after the answer had arrived, because
/// nothing was ever going to stop work that had never started.
///
/// An enum rather than a flag: `prepare_turn(id, None, false)` is unreadable at the call site,
/// and the difference between doing something and looking at it deserves a word.
/// What the map looked like before each edit, and what has been taken back.
///
/// Deliberately not a general-purpose undo stack for the whole application. It holds one kind
/// of thing, it is bounded, and it is emptied when the editor closes — so there is no state in
/// which pressing Undo restores something from a session the user has forgotten.
#[derive(Debug, Default)]
pub struct Timeline {
    /// Newest last.
    done: Vec<epoch_engine::places::WorldMap>,
    /// Newest last. Emptied by any new edit — that branch of history did not happen.
    undone: Vec<epoch_engine::places::WorldMap>,
}

impl Timeline {
    /// How many edits are remembered. Enough to take back a run of mistakes, bounded so a long
    /// editing session cannot grow without limit.
    const DEPTH: usize = 64;

    /// Record the state before an edit.
    fn did(&mut self, before: epoch_engine::places::WorldMap) {
        self.done.push(before);
        if self.done.len() > Self::DEPTH {
            self.done.remove(0);
        }
        // A new edit after an undo abandons the redo branch. Keeping it would offer to restore
        // a future that no longer follows from the present.
        self.undone.clear();
    }

    fn back(
        &mut self,
        current: epoch_engine::places::WorldMap,
    ) -> Option<epoch_engine::places::WorldMap> {
        let previous = self.done.pop()?;
        self.undone.push(current);
        Some(previous)
    }

    fn forward(
        &mut self,
        current: epoch_engine::places::WorldMap,
    ) -> Option<epoch_engine::places::WorldMap> {
        let next = self.undone.pop()?;
        self.done.push(current);
        Some(next)
    }

    fn depth(&self) -> (usize, usize) {
        (self.done.len(), self.undone.len())
    }
}

impl World {
    /// Stop the World so it can be edited (ADR-0028).
    ///
    /// Refused while anybody is working, and the refusal says who. Stricter than deferring
    /// presence transitions until the thaw, and better for the same reason the freeze itself
    /// is: there is no state in which a handoff resolves against a map being rearranged,
    /// because there is no handoff.
    ///
    /// The editor never interrupts work. It waits for it.
    pub fn freeze(&self) -> Result<(), String> {
        if let Some(who) = self.busy() {
            return Err(who);
        }
        if self.lock().world_id.is_none() {
            return Err("no World is open".into());
        }
        self.frozen
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Let the World run again.
    pub fn thaw(&self) {
        self.frozen
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// Whether the World is stopped for editing.
    pub fn is_frozen(&self) -> bool {
        self.frozen.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// This World's map, for the editor to work on.
    pub fn map(&self) -> epoch_engine::places::WorldMap {
        match &self.lock().world_id {
            Some(world) => epoch_engine::places::WorldMap::load(&vault_dir(), world),
            None => epoch_engine::places::WorldMap::default(),
        }
    }

    /// Change the map, and write it.
    ///
    /// Read, edit, write — never a copy held in memory that the file could disagree with. An
    /// editor over the vault, never a parallel store (ADR-0023).
    ///
    /// Refused unless the World is frozen: every one of these moves something a running
    /// character could be standing on.
    pub(crate) fn edit_map<T>(
        &self,
        change: impl FnOnce(&mut epoch_engine::places::WorldMap) -> T,
    ) -> Result<T, String> {
        if !self.is_frozen() {
            return Err("the World has to be stopped before its map can change".into());
        }
        let world = self
            .lock()
            .world_id
            .clone()
            .ok_or_else(|| "no World is open".to_string())?;

        let mut map = epoch_engine::places::WorldMap::load(&vault_dir(), &world);
        let before = map.clone();
        let out = change(&mut map);
        map.save(&vault_dir(), &world).map_err(|e| e.to_string())?;

        // Recorded only once the write succeeded. A snapshot of a change that never reached
        // the file would let Undo restore a state the World was never in.
        if let Ok(mut timeline) = self.edits.lock() {
            timeline.did(before);
        }
        // A building that moved is a different walk. Re-measured here rather than on read
        // because this is the only place a map changes, and a survey taken per read would be a
        // file loaded on every frame.
        self.lock().resurvey();
        Ok(out)
    }

    /// Put the map back the way it was before the last edit.
    ///
    /// Returns whether anything moved, so the surface can say "nothing to undo" rather than
    /// flashing as though it had done something.
    pub fn undo(&self) -> Result<bool, String> {
        self.step(Timeline::back)
    }

    /// Do again what was just undone.
    pub fn redo(&self) -> Result<bool, String> {
        self.step(Timeline::forward)
    }

    /// One move along the timeline: take a snapshot from it, and write it.
    ///
    /// The current map goes onto the other stack as it is written, which is what makes Undo and
    /// Redo each other's inverse rather than two features that agree most of the time.
    pub(crate) fn step(
        &self,
        take: impl FnOnce(
            &mut Timeline,
            epoch_engine::places::WorldMap,
        ) -> Option<epoch_engine::places::WorldMap>,
    ) -> Result<bool, String> {
        if !self.is_frozen() {
            return Err("the World has to be stopped before its map can change".into());
        }
        let world = self
            .lock()
            .world_id
            .clone()
            .ok_or_else(|| "no World is open".to_string())?;

        let current = epoch_engine::places::WorldMap::load(&vault_dir(), &world);
        let restored = {
            let mut timeline = self
                .edits
                .lock()
                .map_err(|_| "the editor lost its history")?;
            take(&mut timeline, current)
        };
        match restored {
            Some(map) => {
                map.save(&vault_dir(), &world).map_err(|e| e.to_string())?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// How many steps back and forward are available, for the surface to show honestly.
    pub fn history_depth(&self) -> (usize, usize) {
        self.edits.lock().map(|t| t.depth()).unwrap_or((0, 0))
    }

    /// Forget this session's edits. Called when the editor closes.
    ///
    /// Undo is an offer to take back what you *just* did. Keeping the stack past the moment the
    /// user left would let a click much later undo something they have stopped thinking about.
    pub fn forget_edits(&self) {
        if let Ok(mut timeline) = self.edits.lock() {
            *timeline = Timeline::default();
        }
    }

    /// Give a Place a different name. Its identity does not move (ADR-0028).
    pub fn rename_place(&self, id: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("give it a name".into());
        }
        let id = epoch_kernel::PlaceId::new(id);
        self.edit_map(|map| map.rename(&id, name))
    }

    /// Build something new, somewhere. Returns its identity.
    ///
    /// The position comes from the surface because only the surface knows how big this World is
    /// and where the user was looking. `None` builds it nowhere — legal, visible as unplaced,
    /// and better than every new building landing on the same coordinate.
    pub fn add_place(&self, name: &str, at: Option<(f32, f32)>) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("give it a name".into());
        }
        let spot = at.map(|(x, y)| epoch_engine::places::Spot {
            x,
            y,
            footprint: None,
        });
        self.edit_map(|map| map.add(name, spot).to_string())
    }

    /// Put a Place somewhere. Its identity does not move; only the building does.
    pub fn move_place(&self, id: &str, x: f32, y: f32) -> Result<(), String> {
        let id = epoch_kernel::PlaceId::new(id);
        self.edit_map(|map| map.place_at(&id, x, y))
    }

    /// Change how big a Place is.
    ///
    /// Refused for one that is nowhere — a size is a fact about a building that is standing
    /// somewhere, and the refusal says which one is not.
    pub fn resize_place(&self, id: &str, footprint: f32) -> Result<(), String> {
        if footprint <= 0.0 {
            return Err("a building has to have a size".into());
        }
        let id = epoch_kernel::PlaceId::new(id);
        match self.edit_map(|map| map.resize(&id, footprint))? {
            true => Ok(()),
            false => Err("put it somewhere first".into()),
        }
    }

    /// Give a building the artwork the user imported, or take it away (ADR-0024).
    ///
    /// The frontend never touches the filesystem: bytes arrive base64 across IPC and only the
    /// Engine writes. The Engine also **names the file from the bytes** — never from the
    /// uploaded name and never from the claimed extension — which removes traversal, spoofing
    /// and collisions instead of defending against them.
    ///
    /// Artwork lives beside `places.toml` in the World's own folder, so exporting a World
    /// carries the buildings its owner drew.
    pub fn set_place_art(&self, id: &str, image: Option<&str>) -> Result<(), String> {
        let world = self
            .lock()
            .world_id
            .clone()
            .ok_or_else(|| "no World is open".to_string())?;
        let dir = epoch_engine::places::WorldMap::art_dir(&vault_dir(), &world);
        let id = epoch_kernel::PlaceId::new(id);
        // A stem per Place, so replacing one building's artwork can never delete another's.
        let stem = format!("place-{id}");

        let file = match image {
            Some(data) => {
                Some(epoch_engine::accept_image(&dir, &stem, data).map_err(|err| err.to_string())?)
            }
            None => {
                epoch_engine::forget_image(&dir, &stem);
                None
            }
        };

        self.edit_map(|map| match file {
            Some(name) => map.redraw(&id, name),
            None => {
                if let Some(entry) = map.places.get_mut(&id) {
                    entry.mark = None;
                }
            }
        })
    }

    /// The land the user painted, as a `data:` URI. `None` when they painted none.
    ///
    /// Its own command rather than a field on the projection: this is megabytes, and the
    /// projection is re-sent on every presence change. Fetched once, on mount and after a
    /// change — the same shape the pack's backdrop already uses.
    pub fn land(&self) -> Option<String> {
        let world = self.lock().world_id.clone()?;
        epoch_engine::places::WorldMap::load(&vault_dir(), &world).land_data(&vault_dir(), &world)
    }

    /// Paint the land, or strip it back to the World's own geography (ADR-0024).
    ///
    /// Authored artwork wins and the derived chart never goes away: clearing brings back the
    /// terrain the World Pack declared, which was there the whole time.
    pub fn set_land(&self, image: Option<&str>) -> Result<(), String> {
        let world = self
            .lock()
            .world_id
            .clone()
            .ok_or_else(|| "no World is open".to_string())?;
        let dir = epoch_engine::places::WorldMap::art_dir(&vault_dir(), &world);

        // Measured from the bytes, like the filename is (ADR-0024). Never from anything the
        // surface claims: an extent supplied by a caller is an extent that can be wrong, and a
        // wrong one silently rescales the user's whole World.
        let (file, size) = match image {
            Some(data) => {
                let name =
                    epoch_engine::accept_image(&dir, "land", data).map_err(|e| e.to_string())?;
                let size = std::fs::read(dir.join(&name))
                    .ok()
                    .and_then(|bytes| {
                        epoch_engine::ImageFormat::sniff(&bytes)
                            .and_then(|format| format.measure(&bytes))
                    })
                    .map(|(w, h)| [w as f32, h as f32]);
                (Some(name), size)
            }
            None => {
                epoch_engine::forget_image(&dir, "land");
                (None, None)
            }
        };

        self.edit_map(|map| map.repaint(file, size))
    }

    /// Take one down, and every road that led to it.
    pub fn remove_place(&self, id: &str) -> Result<(), String> {
        let id = epoch_kernel::PlaceId::new(id);
        self.edit_map(|map| map.remove(&id))
    }

    /// Join two Places, or part them.
    pub fn connect_places(
        &self,
        from: &str,
        to: &str,
        joined: bool,
        via: Option<Vec<[f32; 2]>>,
    ) -> Result<(), String> {
        if from == to {
            return Err("a road has to go somewhere else".into());
        }
        let (from, to) = (
            epoch_kernel::PlaceId::new(from),
            epoch_kernel::PlaceId::new(to),
        );
        self.edit_map(|map| {
            if joined {
                // `None` is the legacy command shape: preserving an existing route is safer
                // than silently flattening it when an older surface only asks to connect.
                if let Some(via) = via {
                    map.connect_with_via(from, to, via);
                } else {
                    map.connect(from, to);
                }
            } else {
                map.roads.retain(|_, r| !r.joins(&from, &to));
            }
        })
    }

    /// Places nothing leads to. The editor's warning, checked while authoring rather than
    /// enforced during a handoff — scenery must never be able to veto work (ADR-0028).
    pub fn unreachable_places(&self) -> Vec<String> {
        // Including the roads this World's pack drew. They are on screen, so they count.
        let by_the_pack: Vec<(String, String)> = self
            .lock()
            .chain
            .map()
            .map(|m| {
                m.routes
                    .iter()
                    .map(|r| (r.from.clone(), r.to.clone()))
                    .collect()
            })
            .unwrap_or_default();
        self.map()
            .unreachable(&by_the_pack)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// The open World's backdrop, read now.
    ///
    /// Its own command rather than a field on `WorldView`: the projection is re-sent whenever
    /// presence changes, and a megabyte illustration has no business crossing IPC every time
    /// somebody turns a page. Fetched once when the World mounts.
    pub fn backdrop(&self) -> Option<String> {
        let inner = self.lock();
        let pack = inner.chain.active()?;
        let (data, problem) = pack.backdrop_data();
        if let Some(problem) = problem {
            eprintln!("[world] {problem}");
        }
        data
    }

    /// Give a World key art, or take it away and go back to its derived chart.
    ///
    /// `None` clears. The image arrives base64-encoded from the webview's own file picker, so
    /// the frontend never gains filesystem access to offer it (see `epoch_engine::import`).
    pub fn set_world_art(&self, world_id: &str, image: Option<&str>) -> Result<(), String> {
        let (packs, _) = WorldPack::discover(&worlds_dir());
        let pack = packs
            .into_iter()
            .find(|p| p.id == world_id)
            .ok_or_else(|| format!("no installed World has the id '{world_id}'"))?;

        match image {
            Some(data) => pack.set_preview(data),
            None => pack.clear_preview(),
        }
        .map_err(|err| err.to_string())
    }

    /// Give the open World its own window artwork, or take it back to Epoch's.
    ///
    /// **The open World, not one the surface names.** The first version took an id, by analogy
    /// with `set_world_art` — which is called from the Launcher, where several Worlds are on
    /// screen and saying which is the whole point. This is called from inside a World's own
    /// editor, where there is exactly one and the Engine is already holding it: `WorldView`
    /// carries no id at all, so asking the frontend for one would mean *giving* it one first,
    /// which is handing the presentation layer a fact it can then get wrong.
    ///
    /// `set_world_land` and `set_place_art` are the neighbours this actually resembles, and
    /// neither takes an id.
    pub fn set_world_skin(
        &self,
        concept: &str,
        image: Option<&str>,
        corner: [u32; 4],
        repeat: &str,
        scale: u32,
    ) -> Result<(), String> {
        let world_id = self
            .lock()
            .world_id
            .clone()
            .ok_or_else(|| "no World is open".to_string())?;
        let (packs, _) = WorldPack::discover(&worlds_dir());
        let pack = packs
            .into_iter()
            .find(|p| p.id == world_id)
            .ok_or_else(|| format!("no installed World has the id '{world_id}'"))?;

        match image {
            Some(data) => pack.set_skin(concept, data, corner, repeat, scale),
            None => pack.clear_skin(concept),
        }
        .map_err(|err| err.to_string())
    }

    /// Give the open World a sound of its own, or take it back to Epoch's synthesised one.
    ///
    /// The open World, for the reason `set_world_skin` is: this is reached from inside a World's
    /// own editor.
    pub fn set_world_sound(&self, concept: &str, audio: Option<&str>) -> Result<(), String> {
        let world_id = self
            .lock()
            .world_id
            .clone()
            .ok_or_else(|| "no World is open".to_string())?;
        let (packs, _) = WorldPack::discover(&worlds_dir());
        let pack = packs
            .into_iter()
            .find(|p| p.id == world_id)
            .ok_or_else(|| format!("no installed World has the id '{world_id}'"))?;

        match audio {
            Some(data) => pack.set_sound(concept, data),
            None => pack.clear_sound(concept),
        }
        .map_err(|err| err.to_string())
    }
}
