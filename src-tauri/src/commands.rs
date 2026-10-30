//! Commands the UI calls through `invoke`. Each returns the data the UI
//! needs to redraw, so the screens never hold stale copies of the show.

use std::time::Duration;

use humlx_core::artnet::Node;
use humlx_core::engine::{self, OutputSettings, OutputStatus};
use humlx_core::fixture::{slug, Attribute, Family, Profile};
use humlx_core::patch::{mode_of, FixtureChanges, NewFixtures};
use humlx_core::programmer::{next_id, Group, Preset, Rgb, SelectMode, Values};
use humlx_core::{ofl, qlc, Error, Result};
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
    highlight: bool,
    /// Fixtures with channel test levels applied.
    tested: Vec<u32>,
    playbacks: Vec<crate::playback::PlaybackStatus>,
    running_effects: Vec<u32>,
    /// Master levels by cue list, which OSC can change too.
    masters: Vec<(u32, f32)>,
    osc: humlx_core::osc::OscStatus,
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
        state.programmer.retain(|id| !ids.contains(&id));
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
    // QLC+ definitions are XML; otherwise a HumLX profile or an OFL export.
    let profile = if json.trim_start().starts_with('<') {
        qlc::import(&json, manufacturer.as_deref())?
    } else {
        match Profile::from_json(&json) {
            Ok(profile) => profile,
            Err(_) => ofl::import(&json, manufacturer.as_deref())?,
        }
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
        highlight: state.highlight,
        tested: state.tested_fixtures(),
        playbacks: crate::playback::statuses(&state),
        running_effects: state.running_effects.ids(),
        masters: crate::playback::masters(&state),
        osc: state.osc.clone(),
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
pub fn set_highlight(app: State<'_, App>, on: bool) {
    app.engine.state().highlight = on;
}

#[tauri::command]
pub async fn discover_nodes() -> Result<Vec<Node>> {
    tauri::async_runtime::spawn_blocking(|| engine::discover(Duration::from_millis(1500)))
        .await
        .map_err(|e| Error::Unsupported(e.to_string()))?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgrammerView {
    selection: Vec<u32>,
    values: std::collections::BTreeMap<u32, Values>,
    groups: Vec<Group>,
    presets: Vec<Preset>,
}

fn programmer_view(app: &App) -> ProgrammerView {
    let state = app.engine.state();
    ProgrammerView {
        selection: state.programmer.selection.clone(),
        values: state.programmer.values.clone(),
        groups: state.groups.clone(),
        presets: state.presets.clone(),
    }
}

/// Changes the programmer, then returns its new state for the UI.
fn change_programmer(app: &App, change: impl FnOnce(&mut engine::State)) -> ProgrammerView {
    change(&mut app.engine.state());
    programmer_view(app)
}

/// Changes groups or presets, saves the show, then returns the programmer state.
fn change_pools(app: &App, change: impl FnOnce(&mut engine::State) -> Result<()>) -> Result<ProgrammerView> {
    change(&mut app.engine.state())?;
    app.save()?;
    Ok(programmer_view(app))
}

#[tauri::command]
pub fn programmer(app: State<'_, App>) -> ProgrammerView {
    programmer_view(&app)
}

#[tauri::command]
pub fn select(app: State<'_, App>, fixtures: Vec<u32>, mode: SelectMode) -> ProgrammerView {
    change_programmer(&app, |state| {
        let patched: Vec<u32> = fixtures
            .into_iter()
            .filter(|id| state.patch.get(*id).is_some())
            .collect();
        state.programmer.select(&patched, mode);
    })
}

#[tauri::command]
pub fn select_group(app: State<'_, App>, id: u32, mode: SelectMode) -> Result<ProgrammerView> {
    let fixtures = {
        let state = app.engine.state();
        let group = state
            .groups
            .iter()
            .find(|g| g.id == id)
            .ok_or(Error::UnknownGroup(id))?;
        group.fixtures.clone()
    };
    Ok(select(app, fixtures, mode))
}

#[tauri::command]
pub fn set_intensity(app: State<'_, App>, value: f32) -> ProgrammerView {
    change_programmer(&app, |state| state.programmer.update(|v| v.intensity = Some(value)))
}

#[tauri::command]
pub fn set_colour(app: State<'_, App>, colour: Rgb) -> ProgrammerView {
    change_programmer(&app, |state| {
        state.programmer.update(|v| {
            v.colour = Some(colour);
            // A picked colour replaces a colour wheel or emitter levels set directly.
            v.other
                .retain(|attribute, _| *attribute != Attribute::ColorWheel && !attribute.is_colour_mix());
        })
    })
}

#[tauri::command]
pub fn set_position(app: State<'_, App>, pan: Option<f32>, tilt: Option<f32>) -> ProgrammerView {
    change_programmer(&app, |state| {
        state.programmer.update(|v| {
            v.pan = pan.or(v.pan);
            v.tilt = tilt.or(v.tilt);
        })
    })
}

#[tauri::command]
pub fn set_attribute(app: State<'_, App>, attribute: Attribute, value: f32) -> ProgrammerView {
    change_programmer(&app, |state| {
        state.programmer.update(|v| {
            if attribute == Attribute::ColorWheel {
                v.colour = None;
            }
            v.other.insert(attribute, value);
        })
    })
}

#[tauri::command]
pub fn clear_programmer(app: State<'_, App>, family: Option<Family>) -> ProgrammerView {
    change_programmer(&app, |state| state.programmer.clear(family))
}

/// Saves the selection as a group, replacing group `id` when given.
#[tauri::command]
pub fn save_group(app: State<'_, App>, name: String, id: Option<u32>) -> Result<ProgrammerView> {
    change_pools(&app, |state| {
        if state.programmer.selection.is_empty() {
            return Err(Error::InvalidShow("select some fixtures first".into()));
        }
        let id = id.unwrap_or_else(|| next_id(state.groups.iter().map(|g| g.id)));
        let group = Group {
            id,
            name: name.trim().to_string(),
            fixtures: state.programmer.selection.clone(),
        };
        state.groups.retain(|g| g.id != id);
        state.groups.push(group);
        state.groups.sort_by_key(|g| g.id);
        Ok(())
    })
}

#[tauri::command]
pub fn delete_group(app: State<'_, App>, id: u32) -> Result<ProgrammerView> {
    change_pools(&app, |state| {
        state.groups.retain(|g| g.id != id);
        Ok(())
    })
}

/// Records the programmer's values for `family` as a preset, replacing preset `id` when given.
#[tauri::command]
pub fn record_preset(app: State<'_, App>, family: Family, name: String, id: Option<u32>) -> Result<ProgrammerView> {
    change_pools(&app, |state| {
        let id = id.unwrap_or_else(|| next_id(state.presets.iter().map(|p| p.id)));
        let preset = Preset::record(id, name.trim().to_string(), family, &state.programmer)?;
        state.presets.retain(|p| p.id != id);
        state.presets.push(preset);
        state.presets.sort_by_key(|p| p.id);
        Ok(())
    })
}

#[tauri::command]
pub fn apply_preset(app: State<'_, App>, id: u32) -> Result<ProgrammerView> {
    {
        let mut state = app.engine.state();
        let state = &mut *state;
        let preset = state
            .presets
            .iter()
            .find(|p| p.id == id)
            .ok_or(Error::UnknownPreset(id))?;
        preset.apply(&mut state.programmer);
    }
    Ok(programmer_view(&app))
}

#[tauri::command]
pub fn delete_preset(app: State<'_, App>, id: u32) -> Result<ProgrammerView> {
    change_pools(&app, |state| {
        state.presets.retain(|p| p.id != id);
        Ok(())
    })
}
