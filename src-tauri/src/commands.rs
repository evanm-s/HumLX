//! Commands the UI calls through `invoke`. Each returns the data the UI
//! needs to redraw, so the screens never hold stale copies of the show.

use std::time::Duration;

use humlx_core::artnet::Node;
use humlx_core::engine::{self, OutputSettings, OutputStatus};
use humlx_core::fixture::{slug, Profile};
use humlx_core::patch::{mode_of, FixtureChanges, NewFixtures};
use humlx_core::{ofl, Error, Result};
use serde::Serialize;
use tauri::State;

use crate::App;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureView {
    id: u32,
    name: String,
    profile: String,
    mode: String,
    address: u16,
    /// Number of channels, or 0 when the profile is missing.
    footprint: usize,
    type_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchView {
    fixtures: Vec<FixtureView>,
    /// Owner of each channel; index 0 is address 1.
    occupancy: Vec<Option<u32>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    id: String,
    #[serde(flatten)]
    profile: Profile,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    profiles: Vec<ProfileView>,
    patch: PatchView,
    output: OutputSettings,
    warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Live {
    universe: Vec<u8>,
    status: OutputStatus,
    blackout: bool,
    /// Fixtures with channel test levels applied.
    tested: Vec<u32>,
}

fn patch_view(app: &App) -> PatchView {
    let state = app.engine.state();
    let fixtures = state
        .patch
        .fixtures()
        .iter()
        .map(|fixture| {
            let profile = state.library.get(&fixture.profile);
            FixtureView {
                id: fixture.id,
                name: fixture.name.clone(),
                profile: fixture.profile.clone(),
                mode: fixture.mode.clone(),
                address: fixture.address,
                footprint: mode_of(&state.library, fixture).map_or(0, |m| m.channels.len()),
                type_name: profile.map_or_else(
                    || format!("Missing type ({})", fixture.profile),
                    |p| format!("{} {}", p.manufacturer, p.model),
                ),
            }
        })
        .collect();
    PatchView {
        fixtures,
        occupancy: state.patch.occupancy(&state.library),
    }
}

fn profile_views(app: &App) -> Vec<ProfileView> {
    let state = app.engine.state();
    state
        .library
        .profiles()
        .iter()
        .map(|profile| ProfileView {
            id: profile.id(),
            profile: profile.clone(),
        })
        .collect()
}

#[tauri::command]
pub fn overview(app: State<'_, App>) -> Overview {
    let warnings = std::mem::take(&mut *app.warnings.lock().unwrap_or_else(|p| p.into_inner()));
    Overview {
        profiles: profile_views(&app),
        patch: patch_view(&app),
        output: app.engine.state().output.clone(),
        warnings,
    }
}

#[tauri::command]
pub fn add_fixtures(app: State<'_, App>, request: NewFixtures) -> Result<PatchView> {
    {
        let mut state = app.engine.state();
        let state = &mut *state;
        state.patch.add(&state.library, &request)?;
    }
    app.save()?;
    Ok(patch_view(&app))
}

#[tauri::command]
pub fn update_fixture(app: State<'_, App>, id: u32, changes: FixtureChanges) -> Result<PatchView> {
    {
        let mut state = app.engine.state();
        let state = &mut *state;
        state.patch.update(&state.library, id, &changes)?;
        // Test levels belong to the old channel layout.
        if changes.mode.is_some() || changes.id.is_some() {
            state.release(Some(id));
        }
    }
    app.save()?;
    Ok(patch_view(&app))
}

#[tauri::command]
pub fn remove_fixtures(app: State<'_, App>, ids: Vec<u32>) -> Result<PatchView> {
    {
        let mut state = app.engine.state();
        state.patch.remove(&ids);
        for id in &ids {
            state.release(Some(*id));
        }
    }
    app.save()?;
    Ok(patch_view(&app))
}

/// Imports a fixture file, either in HumLX's own format or from the Open
/// Fixture Library, and keeps a copy with the show's data.
#[tauri::command]
pub fn import_profile(app: State<'_, App>, json: String, manufacturer: Option<String>) -> Result<Vec<ProfileView>> {
    let profile = match Profile::from_json(&json) {
        Ok(profile) => profile,
        Err(_) => ofl::import(&json, manufacturer.as_deref())?,
    };
    let file = app.paths.fixtures.join(format!(
        "{}--{}.json",
        slug(&profile.manufacturer),
        slug(&profile.model)
    ));
    std::fs::create_dir_all(&app.paths.fixtures)?;
    let json = serde_json::to_string_pretty(&profile).map_err(|e| Error::InvalidProfile(e.to_string()))?;
    std::fs::write(file, json)?;
    app.engine.state().library.insert(profile);
    Ok(profile_views(&app))
}

#[tauri::command]
pub fn set_test_level(app: State<'_, App>, fixture: u32, channel: usize, value: u8) -> Result<()> {
    app.engine.state().set_test_level(fixture, channel, value)
}

#[tauri::command]
pub fn locate(app: State<'_, App>, fixture: u32) -> Result<()> {
    app.engine.state().locate(fixture)
}

#[tauri::command]
pub fn release(app: State<'_, App>, fixture: Option<u32>) {
    app.engine.state().release(fixture);
}

#[tauri::command]
pub fn live(app: State<'_, App>) -> Live {
    let state = app.engine.state();
    Live {
        universe: state.universe().as_bytes().to_vec(),
        status: state.status.clone(),
        blackout: state.blackout,
        tested: state.tested_fixtures(),
    }
}

#[tauri::command]
pub fn set_output(app: State<'_, App>, settings: OutputSettings) -> Result<OutputSettings> {
    settings.validate()?;
    app.engine.state().output = settings.clone();
    app.save()?;
    Ok(settings)
}

#[tauri::command]
pub fn set_blackout(app: State<'_, App>, on: bool) {
    app.engine.state().blackout = on;
}

#[tauri::command]
pub async fn discover_nodes() -> Result<Vec<Node>> {
    tauri::async_runtime::spawn_blocking(|| engine::discover(Duration::from_millis(1500)))
        .await
        .map_err(|e| Error::Unsupported(e.to_string()))?
}
