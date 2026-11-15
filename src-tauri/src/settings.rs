//! Persistent user settings (`settings.json` in the app config directory).

use std::path::PathBuf;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Slovak is the default UI language.
    #[default]
    Sk,
    En,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Persisted {
    pub language: Language,
}

pub struct SettingsStore {
    path: Option<PathBuf>,
    current: RwLock<Persisted>,
}

impl SettingsStore {
    /// In-memory store (tests, or when the config dir is unavailable).
    pub fn in_memory() -> Self {
        Self {
            path: None,
            current: RwLock::new(Persisted::default()),
        }
    }

    /// Load from `path`, falling back to defaults when missing or invalid.
    pub fn load(path: PathBuf) -> Self {
        let current = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            path: Some(path),
            current: RwLock::new(current),
        }
    }

    pub fn get(&self) -> Persisted {
        self.current.read().expect("settings lock").clone()
    }

    pub fn language(&self) -> Language {
        self.get().language
    }

    pub fn update(&self, f: impl FnOnce(&mut Persisted)) -> Result<Persisted, String> {
        let next = {
            let mut cur = self.current.write().expect("settings lock");
            f(&mut cur);
            cur.clone()
        };
        if let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
            }
            let json = serde_json::to_string_pretty(&next).expect("settings serialise");
            std::fs::write(path, json).map_err(|e| format!("cannot save settings: {e}"))?;
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_slovak_and_round_trips() {
        let dir = std::env::temp_dir().join(format!("rgbve-settings-{}", std::process::id()));
        let path = dir.join("settings.json");
        let _ = std::fs::remove_file(&path);

        let store = SettingsStore::load(path.clone());
        assert_eq!(store.language(), Language::Sk);
        store.update(|s| s.language = Language::En).unwrap();
        assert_eq!(SettingsStore::load(path.clone()).language(), Language::En);

        std::fs::write(&path, "not json").unwrap();
        assert_eq!(SettingsStore::load(path).language(), Language::Sk);
        let _ = std::fs::remove_dir_all(dir);
    }
}
