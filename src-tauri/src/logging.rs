//! File logging. The app never opens a console window, so diagnostics are
//! written to `<app log dir>/rgb-visual-enhancer.log` (and to stderr when a
//! terminal is attached, e.g. `npm run tauri dev`).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "rgb-visual-enhancer.log";
/// Rotate when the log grows past this size.
const MAX_BYTES: u64 = 2 * 1024 * 1024;

struct Tee {
    file: Option<File>,
}

impl Write for Tee {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(f) = &mut self.file {
            f.write_all(buf)?;
        }
        // Ignored when no console is attached.
        let _ = io::stderr().write_all(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(f) = &mut self.file {
            f.flush()?;
        }
        Ok(())
    }
}

fn open_log(dir: &Path) -> io::Result<(File, PathBuf)> {
    fs::create_dir_all(dir)?;
    let path = dir.join(FILE_NAME);
    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = fs::rename(&path, dir.join(format!("{FILE_NAME}.old")));
    }
    let file = OpenOptions::new().create(true).append(true).open(&path)?;
    Ok((file, path))
}

/// Initialise logging; returns the log file path when one could be opened.
pub fn init(dir: Option<PathBuf>) -> Option<PathBuf> {
    let opened = dir.and_then(|d| open_log(&d).ok());
    let path = opened.as_ref().map(|(_, p)| p.clone());
    let tee = Tee {
        file: opened.map(|(f, _)| f),
    };
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(Box::new(tee)))
        .try_init();
    path
}
