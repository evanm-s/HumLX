//! Tauri shell around `humlx-core`: starts the engine, loads and saves the
//! show, and exposes commands to the UI.

mod commands;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use humlx_core::engine::{Engine, State};
use humlx_core::fixture::{Library, Profile};
use humlx_core::show::Show;
use tauri::Manager;

pub struct App {
    engine: Engine,
    paths: Paths,
    /// Problems found while loading, shown once in the UI.
    warnings: Mutex<Vec<String>>,
}

struct Paths {
    show: PathBuf,
    fixtures: PathBuf,
}

impl App {
    fn load(data_dir: &Path) -> std::io::Result<Self> {
        let paths = Paths {
            show: data_dir.join("show.json"),
            fixtures: data_dir.join("fixtures"),
        };
        let mut warnings = Vec::new();

        let mut library = Library::with_bundled();
        for (file, result) in load_profiles(&paths.fixtures) {
            match result {
                Ok(profile) => library.insert(profile),
                Err(error) => warnings.push(format!("Skipped fixture file {file}: {error}")),
            }
        }

        let show = if paths.show.exists() {
            Show::load(&paths.show).unwrap_or_else(|error| {
                // Keep the broken file for recovery rather than overwriting it.
                let backup = paths.show.with_extension("broken.json");
                let _ = std::fs::rename(&paths.show, &backup);
                warnings.push(format!(
                    "The saved show could not be opened ({error}). It was moved to {} and a new show was started.",
                    backup.display()
                ));
                Show::default()
            })
        } else {
            Show::default()
        };

        let engine = Engine::start(State::new(library, show.patch, show.output))?;
        Ok(Self {
            engine,
            paths,
            warnings: Mutex::new(warnings),
        })
    }

    /// Saves the patch and output settings after every change.
    fn save(&self) -> humlx_core::Result<()> {
        let show = {
            let state = self.engine.state();
            Show {
                patch: state.patch.clone(),
                output: state.output.clone(),
                ..Show::default()
            }
        };
        show.save(&self.paths.show)
    }
}

fn load_profiles(dir: &Path) -> Vec<(String, humlx_core::Result<Profile>)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .map(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let result = std::fs::read_to_string(&path)
                .map_err(Into::into)
                .and_then(|json| Profile::from_json(&json));
            (name, result)
        })
        .collect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(App::load(&data_dir)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::overview,
            commands::add_fixtures,
            commands::update_fixture,
            commands::remove_fixtures,
            commands::import_profile,
            commands::set_test_level,
            commands::locate,
            commands::release,
            commands::live,
            commands::set_output,
            commands::set_blackout,
            commands::discover_nodes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running HumLX");
}
