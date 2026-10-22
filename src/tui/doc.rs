//! What the TUI edits: the config and the model registry, with undo
//! history and the on-disk state they were loaded from, so a save never
//! silently overwrites a file changed elsewhere (the in-app pane, an
//! editor).

use crate::config::{self, Config, LoadNote};
use crate::models::{ModelsFile, Registry};
use crate::presets::Library;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Undo steps kept.
const HISTORY: usize = 200;

#[derive(Clone, Debug)]
pub struct Paths {
    pub config: PathBuf,
    /// Where a migrated v1 config is backed up.
    pub config_backup: PathBuf,
    pub models: PathBuf,
    /// The user's own presets.
    pub presets: PathBuf,
}

impl Paths {
    pub fn user() -> Paths {
        Paths {
            config: crate::paths::config_file(),
            config_backup: crate::paths::legacy_config_backup(),
            models: crate::paths::models_file(),
            presets: crate::paths::presets_dir(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    config: Config,
    models: ModelsFile,
}

#[derive(Debug, PartialEq)]
pub enum SaveError {
    /// These files changed on disk since they were loaded.
    Conflict(Vec<PathBuf>),
    Io(String),
}

pub struct Doc {
    pub config: Config,
    pub models: ModelsFile,
    /// The presets on offer. Changes to the user's are written at once,
    /// outside the undo history.
    pub presets: Library,
    saved: Snapshot,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// Edits since the last save, for the quit prompt.
    edits: usize,
    paths: Paths,
    /// Modification times of the files as last loaded or saved.
    stamps: [Option<SystemTime>; 2],
    /// The config file is still in the v1 format; saving replaces it.
    v1: bool,
    pub note: LoadNote,
    /// Why `models.toml` couldn't be read; the registry starts empty then.
    pub models_error: Option<String>,
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl Doc {
    /// Read both files. Opening never writes: a v1 config is converted in
    /// memory and replaced only when saved.
    pub fn open(paths: Paths) -> Doc {
        let (config, note) = config::read_from(&paths.config);
        let (models, models_error) = match ModelsFile::load(&paths.models) {
            Ok(m) => (m, None),
            Err(e) => (ModelsFile::default(), Some(e)),
        };
        let saved = Snapshot {
            config: config.clone(),
            models: models.clone(),
        };
        Doc {
            config,
            models,
            presets: Library::load(paths.presets.clone()),
            saved,
            undo: Vec::new(),
            redo: Vec::new(),
            edits: 0,
            stamps: [mtime(&paths.config), mtime(&paths.models)],
            v1: note == LoadNote::V1,
            paths,
            note,
            models_error,
        }
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    fn current(&self) -> Snapshot {
        Snapshot {
            config: self.config.clone(),
            models: self.models.clone(),
        }
    }

    fn restore(&mut self, s: Snapshot) {
        self.config = s.config;
        self.models = s.models;
    }

    /// Apply one user action; it becomes one undo step if it changed
    /// anything.
    pub fn edit(&mut self, f: impl FnOnce(&mut Config, &mut ModelsFile)) {
        let before = self.current();
        f(&mut self.config, &mut self.models);
        if self.current() != before {
            self.undo.push(before);
            if self.undo.len() > HISTORY {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.edits += 1;
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.current());
        self.restore(prev);
        self.edits = if self.dirty() {
            self.edits.saturating_sub(1).max(1)
        } else {
            0
        };
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.current());
        self.restore(next);
        self.edits = if self.dirty() { self.edits + 1 } else { 0 };
        true
    }

    pub fn config_dirty(&self) -> bool {
        self.v1 || self.config != self.saved.config
    }

    pub fn models_dirty(&self) -> bool {
        self.models != self.saved.models
    }

    pub fn dirty(&self) -> bool {
        self.config_dirty() || self.models_dirty()
    }

    pub fn edits(&self) -> usize {
        if self.dirty() { self.edits.max(1) } else { 0 }
    }

    /// Write the changed files. Unless `force`, refuses when a file changed
    /// on disk since it was loaded or saved.
    pub fn save(&mut self, force: bool) -> Result<(), SaveError> {
        let targets = [
            (self.config_dirty(), self.paths.config.clone()),
            (self.models_dirty(), self.paths.models.clone()),
        ];
        if !force {
            let changed: Vec<PathBuf> = targets
                .iter()
                .zip(self.stamps)
                .filter(|((dirty, path), stamp)| *dirty && mtime(path) != *stamp)
                .map(|((_, path), _)| path.clone())
                .collect();
            if !changed.is_empty() {
                return Err(SaveError::Conflict(changed));
            }
        }
        let io = |e: std::io::Error| SaveError::Io(e.to_string());
        if targets[0].0 {
            if self.v1 && !self.paths.config_backup.exists() {
                std::fs::copy(&self.paths.config, &self.paths.config_backup).map_err(io)?;
            }
            self.config.save(&self.paths.config).map_err(io)?;
            self.v1 = false;
            self.note = LoadNote::Fine;
            self.saved.config = self.config.clone();
            self.stamps[0] = mtime(&self.paths.config);
        }
        if targets[1].0 {
            self.models.save(&self.paths.models).map_err(io)?;
            self.saved.models = self.models.clone();
            self.stamps[1] = mtime(&self.paths.models);
        }
        self.edits = 0;
        Ok(())
    }

    /// Discard edits and history and read the files again.
    pub fn reload(&mut self) {
        *self = Doc::open(self.paths.clone());
    }

    pub fn registry(&self) -> Registry {
        Registry::from_file(self.models.clone())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::models::Rule;

    /// Paths in a fresh temporary directory; removed by the caller.
    pub fn temp_paths(tag: &str) -> (PathBuf, Paths) {
        let dir = std::env::temp_dir().join(format!("ccline-doc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let paths = Paths {
            config: dir.join("config.toml"),
            config_backup: dir.join("config.v1.toml"),
            models: dir.join("models.toml"),
            presets: dir.join("presets"),
        };
        (dir, paths)
    }

    fn rule(pattern: &str) -> Rule {
        Rule {
            pattern: pattern.into(),
            display_name: Some(pattern.to_uppercase()),
            context_limit: None,
            match_kind: Default::default(),
        }
    }

    #[test]
    fn edits_undo_redo_and_track_dirtiness() {
        let (dir, paths) = temp_paths("undo");
        let mut doc = Doc::open(paths);
        assert_eq!(doc.note, LoadNote::Missing);
        assert!(!doc.dirty());

        doc.edit(|c, _| c.palette = "nord".into());
        doc.edit(|_, m| m.rules.push(rule("kimi")));
        doc.edit(|_, _| {});
        assert_eq!(doc.edits(), 2);
        assert!(doc.config_dirty() && doc.models_dirty());

        assert!(doc.undo());
        assert!(doc.models.rules.is_empty());
        assert!(doc.undo());
        assert!(!doc.dirty() && doc.edits() == 0);
        assert!(!doc.undo());
        assert!(doc.redo());
        assert_eq!(doc.config.palette, "nord");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saves_only_changed_files_and_reads_them_back() {
        let (dir, paths) = temp_paths("save");
        let mut doc = Doc::open(paths.clone());
        doc.edit(|_, m| m.rules.push(rule("kimi")));
        doc.save(false).unwrap();
        assert!(!doc.dirty());
        assert!(paths.models.exists());
        assert!(!paths.config.exists());
        let again = Doc::open(paths);
        assert_eq!(again.models.rules[0].pattern, "kimi");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn replaces_a_v1_config_only_on_save() {
        let (dir, paths) = temp_paths("v1");
        let v1 = "theme = \"nord\"\n[style]\nmode = \"nerd_font\"\n";
        std::fs::write(&paths.config, v1).unwrap();
        let mut doc = Doc::open(paths.clone());
        assert_eq!(doc.note, LoadNote::V1);
        assert_eq!(doc.config.palette, "nord");
        // Opening wrote nothing, but the conversion is an unsaved change.
        assert_eq!(std::fs::read_to_string(&paths.config).unwrap(), v1);
        assert!(!paths.config_backup.exists());
        assert!(doc.dirty());
        doc.save(false).unwrap();
        assert_eq!(std::fs::read_to_string(&paths.config_backup).unwrap(), v1);
        assert_eq!(Doc::open(paths).note, LoadNote::Fine);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_outside_changes() {
        let (dir, paths) = temp_paths("conflict");
        let mut doc = Doc::open(paths.clone());
        // Another writer creates the file after the TUI opened.
        std::fs::write(&paths.config, "version = 2\npalette = \"dracula\"\n").unwrap();
        doc.edit(|c, _| c.palette = "nord".into());
        assert_eq!(
            doc.save(false),
            Err(SaveError::Conflict(vec![paths.config.clone()]))
        );
        doc.reload();
        assert_eq!(doc.config.palette, "dracula");
        doc.edit(|c, _| c.palette = "nord".into());
        doc.save(false).unwrap();
        assert_eq!(Doc::open(paths).config.palette, "nord");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
