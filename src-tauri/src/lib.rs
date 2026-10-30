//! Tauri shell around `humlx-core`: starts the engine, loads and saves the
//! show, and exposes commands to the UI.

mod commands;
mod playback;
mod shows;
mod surface;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use humlx_core::engine::{Engine, State};
use humlx_core::fixture::{bundled, Library, Profile};
use humlx_core::osc::Listener;
use humlx_core::show::Show;
use tauri::Manager;

pub struct App {
    engine: Engine,
    paths: Paths,
    /// Problems found while loading, shown once in the UI.
    warnings: Mutex<Vec<String>>,
    /// Receives OSC while enabled; replaced when its settings change.
    osc: Mutex<Option<Listener>>,
    /// The show file last saved or opened, if any.
    show_file: Mutex<Option<PathBuf>>,
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

        let mut state = State::new(library, Default::default(), Default::default());
        apply_show(&mut state, show);
        let engine = Engine::start(state)?;
        let app = Self {
            engine,
            paths,
            warnings: Mutex::new(warnings),
            osc: Mutex::new(None),
            show_file: Mutex::new(None),
        };
        app.restart_osc();
        Ok(app)
    }

    /// The show as it stands, with the non-built-in fixture types it uses.
    fn current_show(&self) -> Show {
        let state = self.engine.state();
        let builtin: Vec<String> = bundled().iter().map(Profile::id).collect();
        let used: std::collections::BTreeSet<&str> =
            state.patch.fixtures().iter().map(|f| f.profile.as_str()).collect();
        let profiles = used
            .into_iter()
            .filter(|id| !builtin.iter().any(|b| b == id))
            .filter_map(|id| state.library.get(id).cloned())
            .collect();
        Show {
            patch: state.patch.clone(),
            output: state.output.clone(),
            groups: state.groups.clone(),
            presets: state.presets.clone(),
            cue_lists: state.cue_lists.clone(),
            effects: state.effects.clone(),
            osc: state.osc_settings.clone(),
            highlight_colour: state.highlight_colour,
            profiles,
            ..Show::default()
        }
    }

    /// Saves the show after every change.
    fn save(&self) -> humlx_core::Result<()> {
        self.current_show().save(&self.paths.show)
    }

    /// (Re)starts the OSC listener from the current settings.
    fn restart_osc(&self) {
        let mut osc = self.osc.lock().unwrap_or_else(|p| p.into_inner());
        // Stop the old listener first so its port is free.
        *osc = None;
        let settings = self.engine.state().osc_settings.clone();
        *osc = Listener::start(self.engine.shared(), &settings);
    }
}

/// Replaces everything in the state with a show, keeping the fixture library
/// and adding the show's own fixture types to it.
fn apply_show(state: &mut State, show: Show) {
    for profile in show.profiles {
        if state.library.get(&profile.id()).is_none() {
            state.library.insert(profile);
        }
    }
    state.patch = show.patch;
    state.output = show.output;
    state.groups = show.groups;
    state.presets = show.presets;
    state.cue_lists = show.cue_lists;
    state.effects = show.effects;
    state.osc_settings = show.osc;
    state.highlight_colour = show.highlight_colour;
    state.programmer = Default::default();
    state.playbacks.release_all();
    state.running_effects.stop_all();
    state.blackout = false;
    state.highlight = false;
    state.release(None);
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(App::load(&data_dir)?);
            app.manage(surface::start(app.handle().clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::overview,
            commands::add_fixtures,
            commands::update_fixture,
            commands::update_fixtures,
            commands::remove_fixtures,
            commands::import_profile,
            commands::set_test_level,
            commands::locate,
            commands::release,
            commands::live,
            commands::set_output,
            commands::set_blackout,
            commands::set_highlight,
            commands::highlight_colour,
            commands::set_highlight_colour,
            commands::discover_nodes,
            commands::programmer,
            commands::select,
            commands::select_group,
            commands::set_intensity,
            commands::set_colour,
            commands::set_position,
            commands::set_attribute,
            commands::clear_programmer,
            commands::save_group,
            commands::delete_group,
            commands::record_preset,
            commands::apply_preset,
            commands::delete_preset,
            playback::playback,
            playback::new_cue_list,
            playback::rename_cue_list,
            playback::delete_cue_list,
            playback::record_cue,
            shows::osc_settings,
            shows::set_osc,
            shows::show_file,
            shows::save_show_as,
            shows::open_show,
            shows::new_show,
            surface::surface_status,
            surface::set_surface_leds,
            playback::update_cue,
            playback::edit_cue,
            playback::delete_cue,
            playback::go,
            playback::back,
            playback::go_to,
            playback::release_cue_list,
            playback::release_all,
            playback::set_master,
            playback::save_effect,
            playback::delete_effect,
            playback::set_effect_running,
        ])
        .run(tauri::generate_context!())
        .expect("error while running HumLX");
}
