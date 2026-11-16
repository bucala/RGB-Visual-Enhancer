//! Lighting profiles and automatic switching when a game starts or exits.
//!
//! Running processes are read through the OS API (Toolhelp32 on Windows,
//! `/proc` on Linux) — no helper programs or console windows are spawned.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use rgb_core::DeviceManager;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::settings::{Profile, SettingsStore};

pub const PROFILE_CHANNEL: &str = "rgb://profile";
const POLL: Duration = Duration::from_secs(3);

/// Which profile is currently applied and why.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ActiveProfile {
    pub id: Option<String>,
    /// The game executable that triggered it, for automatic switches.
    pub game: Option<String>,
}

#[derive(Default)]
pub struct ProfileState {
    pub active: Mutex<ActiveProfile>,
}

/// Apply a profile to every connected device.
pub async fn apply(manager: &DeviceManager, profile: &Profile) -> Vec<(String, String)> {
    let devices = manager.devices().await;
    let mut failures = Vec::new();
    if let Some(color) = profile.color {
        let ids: Vec<String> = devices.iter().map(|d| d.info.id.clone()).collect();
        failures.extend(manager.set_color_many(&ids, color).await);
    }
    if !profile.keys.is_empty() {
        let keys: Vec<_> = profile.keys.iter().map(|(k, c)| (k.clone(), *c)).collect();
        for d in devices.iter().filter(|d| d.info.layout.is_some()) {
            if let Err(e) = manager.set_key_colors(&d.info.id, keys.clone()).await {
                failures.push((d.info.id.clone(), e));
            }
        }
    }
    failures
}

pub async fn apply_and_announce<R: Runtime>(
    app: &AppHandle<R>,
    manager: &DeviceManager,
    profile: &Profile,
    game: Option<String>,
) {
    for (id, e) in apply(manager, profile).await {
        log::warn!("profile '{}' on {id}: {e}", profile.name);
    }
    let active = ActiveProfile {
        id: Some(profile.id.clone()),
        game,
    };
    if let Some(state) = app.try_state::<ProfileState>() {
        *state.active.lock().expect("profile state") = active.clone();
    }
    let _ = app.emit(PROFILE_CHANNEL, active);
}

/// Lower-case executable names of running processes.
pub fn running_processes() -> HashSet<String> {
    platform::processes()
        .into_iter()
        .map(|p| p.to_lowercase())
        .collect()
}

/// The profile that should be active: the first one whose game is running,
/// else the default profile.
pub fn desired<'a>(
    profiles: &'a [Profile],
    default: Option<&str>,
    running: &HashSet<String>,
) -> Option<(&'a Profile, Option<String>)> {
    for p in profiles {
        if let Some(game) = p.games.iter().find(|g| running.contains(&g.to_lowercase())) {
            return Some((p, Some(game.clone())));
        }
    }
    default
        .and_then(|id| profiles.iter().find(|p| p.id == id))
        .map(|p| (p, None))
}

/// Background task: apply the default profile at startup and follow games.
pub fn spawn_watcher<R: Runtime>(app: AppHandle<R>, manager: DeviceManager) {
    tauri::async_runtime::spawn(async move {
        // Give the initial hardware scan a head start.
        tokio::time::sleep(Duration::from_secs(4)).await;
        let mut last: Option<(String, Option<String>)> = None;
        loop {
            if let Some(store) = app.try_state::<SettingsStore>() {
                let settings = store.get();
                let running = if settings.auto_switch {
                    tokio::task::spawn_blocking(running_processes)
                        .await
                        .unwrap_or_default()
                } else {
                    HashSet::new()
                };
                let want = desired(
                    &settings.profiles,
                    settings.default_profile.as_deref(),
                    &running,
                );
                let key = want.as_ref().map(|(p, g)| (p.id.clone(), g.clone()));
                // Only act on changes, so manual choices aren't overridden
                // every few seconds.
                if key != last {
                    if let Some((profile, game)) = want {
                        log::info!(
                            "applying profile '{}'{}",
                            profile.name,
                            game.as_deref()
                                .map(|g| format!(" for {g}"))
                                .unwrap_or_default()
                        );
                        apply_and_announce(&app, &manager, profile, game).await;
                    }
                    last = key;
                }
            }
            tokio::time::sleep(POLL).await;
        }
    });
}

#[cfg(windows)]
mod platform {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    pub fn processes() -> Vec<String> {
        let mut out = Vec::new();
        // SAFETY: standard Toolhelp32 enumeration; the entry struct is sized
        // before use and the snapshot handle is always closed.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return out;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut ok = Process32FirstW(snap, &mut entry) != 0;
            while ok {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                out.push(String::from_utf16_lossy(&entry.szExeFile[..len]));
                ok = Process32NextW(snap, &mut entry) != 0;
            }
            CloseHandle(snap);
        }
        out
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn processes() -> Vec<String> {
        let Ok(dir) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        dir.flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .bytes()
                    .all(|b| b.is_ascii_digit())
            })
            .filter_map(|e| {
                std::fs::read_link(e.path().join("exe"))
                    .ok()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .or_else(|| {
                        std::fs::read_to_string(e.path().join("comm"))
                            .ok()
                            .map(|s| s.trim().to_string())
                    })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn profile(id: &str, games: &[&str]) -> Profile {
        Profile {
            id: id.into(),
            name: id.into(),
            games: games.iter().map(|g| g.to_string()).collect(),
            color: None,
            keys: BTreeMap::new(),
        }
    }

    #[test]
    fn picks_running_game_then_default() {
        let profiles = vec![profile("fps", &["CS2.exe"]), profile("work", &[])];
        let running: HashSet<String> = ["explorer.exe", "cs2.exe"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (p, game) = desired(&profiles, Some("work"), &running).unwrap();
        assert_eq!((p.id.as_str(), game.as_deref()), ("fps", Some("CS2.exe")));

        let idle: HashSet<String> = ["explorer.exe".to_string()].into();
        let (p, game) = desired(&profiles, Some("work"), &idle).unwrap();
        assert_eq!((p.id.as_str(), game), ("work", None));
        assert!(desired(&profiles, None, &idle).is_none());
        assert!(desired(&profiles, Some("missing"), &idle).is_none());
    }

    #[test]
    fn lists_this_process() {
        let procs = running_processes();
        assert!(!procs.is_empty(), "process list should not be empty");
    }
}
