//! Persistent user settings (`settings.json` in the app config directory).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::RwLock;

use rgb_core::Rgb;
use serde::{Deserialize, Serialize};

/// UI languages (BCP 47 tags). Slovak is the default.
pub const LANGUAGES: [&str; 25] = [
    "sk", "en", "cs", "hu", "es-ES", "es-419", "zh-Hans", "zh-Hant", "de", "fr", "ja", "pt-BR",
    "pt-PT", "ko", "ar", "hi", "it", "id", "vi", "th", "tr", "pl", "uk", "nl", "sv",
];

/// A validated UI language tag.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Language(String);

impl Language {
    pub fn sk() -> Self {
        Self("sk".into())
    }

    pub fn en() -> Self {
        Self("en".into())
    }

    pub fn code(&self) -> &str {
        &self.0
    }
}

impl Default for Language {
    fn default() -> Self {
        Self::sk()
    }
}

impl TryFrom<String> for Language {
    type Error = String;

    fn try_from(s: String) -> Result<Self, String> {
        LANGUAGES
            .iter()
            .find(|l| l.eq_ignore_ascii_case(&s))
            .map(|l| Language((*l).to_string()))
            .ok_or_else(|| format!("unsupported language '{s}'"))
    }
}

impl From<Language> for String {
    fn from(l: Language) -> String {
        l.0
    }
}

/// A saved lighting setup, optionally tied to games.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// Executable names (e.g. `cs2.exe`); the profile is applied while one runs.
    #[serde(default)]
    pub games: Vec<String>,
    /// Colour applied to every device first.
    #[serde(default)]
    pub color: Option<Rgb>,
    /// Per-key colours for keyboards with a key layout.
    #[serde(default)]
    pub keys: BTreeMap<String, Rgb>,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Persisted {
    pub language: Language,
    pub profiles: Vec<Profile>,
    /// Applied at startup and whenever no game profile is active.
    pub default_profile: Option<String>,
    /// Switch profiles automatically when a listed game starts or exits.
    #[serde(default = "yes")]
    pub auto_switch: bool,
}

impl Default for Persisted {
    fn default() -> Self {
        Self {
            language: Language::default(),
            profiles: Vec::new(),
            default_profile: None,
            auto_switch: true,
        }
    }
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
        assert_eq!(store.language(), Language::sk());
        assert!(store.get().auto_switch);
        store
            .update(|s| {
                s.language = Language::try_from("zh-Hant".to_string()).unwrap();
                s.profiles.push(Profile {
                    id: "fps".into(),
                    name: "FPS".into(),
                    games: vec!["cs2.exe".into()],
                    color: None,
                    keys: [("k:1a".to_string(), Rgb::new(255, 0, 0))].into(),
                });
            })
            .unwrap();
        let reloaded = SettingsStore::load(path.clone()).get();
        assert_eq!(reloaded.language.code(), "zh-Hant");
        assert_eq!(reloaded.profiles[0].keys["k:1a"], Rgb::new(255, 0, 0));

        // Older files without the new fields still load.
        std::fs::write(&path, r#"{"language":"en"}"#).unwrap();
        let old = SettingsStore::load(path.clone()).get();
        assert_eq!(old.language, Language::en());
        assert!(old.auto_switch && old.profiles.is_empty());

        std::fs::write(&path, "not json").unwrap();
        assert_eq!(SettingsStore::load(path).language(), Language::sk());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn language_validation() {
        assert!(Language::try_from("de".to_string()).is_ok());
        assert_eq!(
            Language::try_from("pt-br".to_string()).unwrap().code(),
            "pt-BR"
        );
        assert!(Language::try_from("xx".to_string()).is_err());
    }
}
