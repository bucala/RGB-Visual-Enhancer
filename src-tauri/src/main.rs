// Always a GUI-subsystem app on Windows, also in debug builds: no console
// window ever pops up. Logs go to a file instead (see `logging.rs`).
#![windows_subsystem = "windows"]

fn main() {
    rgb_visual_enhancer_lib::run()
}
