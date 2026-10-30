//! Commands for show files and OSC input settings.

use std::path::PathBuf;

use humlx_core::fixture::slug;
use humlx_core::osc::OscSettings;
use humlx_core::show::Show;
use humlx_core::{Error, Result};
use tauri::State;

use crate::{apply_show, App};

#[tauri::command]
pub fn osc_settings(app: State<'_, App>) -> OscSettings {
    app.engine.state().osc_settings.clone()
}

#[tauri::command]
pub fn set_osc(app: State<'_, App>, settings: OscSettings) -> Result<OscSettings> {
    if settings.port == 0 {
        return Err(Error::InvalidOsc("choose a port from 1 to 65535".into()));
    }
    app.engine.state().osc_settings = settings.clone();
    app.save()?;
    app.restart_osc();
    Ok(settings)
}

/// The file the show was last saved to or opened from, if any.
#[tauri::command]
pub fn show_file(app: State<'_, App>) -> Option<String> {
    current_file(&app).map(|p| p.display().to_string())
}

#[tauri::command]
pub fn save_show_as(app: State<'_, App>, path: String) -> Result<String> {
    let path = with_extension(PathBuf::from(path));
    app.current_show().save(&path)?;
    let shown = path.display().to_string();
    *app.show_file.lock().unwrap_or_else(|p| p.into_inner()) = Some(path);
    Ok(shown)
}

/// Replaces the current show with one from a file. Fixture types inside it
/// that this computer doesn't have are kept for next time.
#[tauri::command]
pub fn open_show(app: State<'_, App>, path: String) -> Result<String> {
    let path = PathBuf::from(path);
    let show = Show::load(&path)?;
    std::fs::create_dir_all(&app.paths.fixtures)?;
    for profile in &show.profiles {
        let file = app.paths.fixtures.join(format!(
            "{}--{}.json",
            slug(&profile.manufacturer),
            slug(&profile.model)
        ));
        if !file.exists() {
            let json = serde_json::to_string_pretty(profile).map_err(|e| Error::InvalidProfile(e.to_string()))?;
            std::fs::write(file, json)?;
        }
    }
    apply_show(&mut app.engine.state(), show);
    app.save()?;
    app.restart_osc();
    let shown = path.display().to_string();
    *app.show_file.lock().unwrap_or_else(|p| p.into_inner()) = Some(path);
    Ok(shown)
}

/// Starts an empty show, keeping the output, OSC and highlight settings for this studio.
#[tauri::command]
pub fn new_show(app: State<'_, App>) -> Result<()> {
    {
        let mut state = app.engine.state();
        let show = Show {
            output: state.output.clone(),
            osc: state.osc_settings.clone(),
            highlight_colour: state.highlight_colour,
            ..Show::default()
        };
        apply_show(&mut state, show);
    }
    app.save()?;
    *app.show_file.lock().unwrap_or_else(|p| p.into_inner()) = None;
    Ok(())
}

fn current_file(app: &App) -> Option<PathBuf> {
    app.show_file.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

fn with_extension(path: PathBuf) -> PathBuf {
    if path.extension().is_some() {
        path
    } else {
        path.with_extension("humlx")
    }
}
