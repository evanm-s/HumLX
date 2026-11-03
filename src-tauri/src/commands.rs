//! Commands the UI calls through `invoke`. Each returns the data the UI
//! needs to redraw, so the screens never hold stale copies of the show.

use std::time::Duration;

use humlx_core::artnet::Node;
use humlx_core::engine::{self, OutputSettings, OutputStatus};
use humlx_core::fixture::{slug, Attribute, Family, Profile};
use humlx_core::patch::{mode_of, FixtureChanges, FixtureOptions, FixtureUpdate, NewFixtures};
use humlx_core::programmer::{next_id, Field, Group, Preset, Rgb, SelectMode, Values};
use humlx_core::stage::{Placed, Plan, Stage};
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
    options: FixtureOptions,
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
    /// Grand master level from 0 to 1, which OSC can change too.
    grand_master: f32,
    highlight: bool,
    /// Fixtures with channel test levels applied.
    tested: Vec<u32>,
    playbacks: Vec<crate::playback::PlaybackStatus>,
    /// Master levels by cue list, which OSC can change too.
    masters: Vec<(u32, f32)>,
    /// Tempo of speed masters A and B, which OSC can change too.
    speed_bpm: [f32; 2],
    /// Beats counted by speed masters A and B, for their beat lights.
    beats: [f32; 2],
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
                options: fixture.options,
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
pub fn update_fixtures(app: State<'_, App>, updates: Vec<FixtureUpdate>) -> Result<PatchView> {
    {
        let mut state = app.engine.state();
        let state = &mut *state;
        state.patch.update_many(&state.library, &updates)?;
        for update in &updates {
            if update.changes.mode.is_some() || update.changes.id.is_some() {
                state.release(Some(update.id));
            }
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
        grand_master: state.grand_master.0,
        highlight: state.highlight,
        tested: state.tested_fixtures(),
        playbacks: crate::playback::statuses(&state),
        masters: crate::playback::masters(&state),
        speed_bpm: state.speed_masters.bpms(),
        beats: state.speed_masters.beats(std::time::Instant::now()).0,
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
pub fn set_grand_master(app: State<'_, App>, level: f32) {
    app.engine.state().grand_master.set(level);
}

#[tauri::command]
pub fn set_highlight(app: State<'_, App>, on: bool) {
    app.engine.state().highlight = on;
}

#[tauri::command]
pub fn highlight_colour(app: State<'_, App>) -> Rgb {
    app.engine.state().highlight_colour
}

#[tauri::command]
pub fn set_highlight_colour(app: State<'_, App>, colour: Rgb) -> Result<Rgb> {
    let colour = colour.clamped();
    app.engine.state().highlight_colour = colour;
    app.save()?;
    Ok(colour)
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
    effects: Vec<humlx_core::effect::Effect>,
    groups: Vec<Group>,
    presets: Vec<Preset>,
}

fn programmer_view(app: &App) -> ProgrammerView {
    let state = app.engine.state();
    ProgrammerView {
        selection: state.programmer.selection.clone(),
        values: state.programmer.values.clone(),
        effects: state.programmer.effects.clone(),
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
pub fn clear_programmer(app: State<'_, App>, family: Option<Family>, fixtures: Option<Vec<u32>>) -> ProgrammerView {
    change_programmer(&app, |state| match &fixtures {
        Some(fixtures) => state.programmer.clear_fixtures(family, fixtures),
        None => state.programmer.clear(family),
    })
}

/// Takes whole fixtures (`rows`) and single values (`cells`) out of the
/// programmer, as picked on the programmer sheet.
#[tauri::command]
pub fn remove_from_programmer(app: State<'_, App>, rows: Vec<u32>, cells: Vec<(u32, Field)>) -> ProgrammerView {
    change_programmer(&app, |state| {
        state.programmer.remove_values(&cells);
        if !rows.is_empty() {
            state.programmer.clear_fixtures(None, &rows);
        }
    })
}

/// Load: copies what the running cue lists put out for `fixtures` (every
/// fixture when `None`) into the programmer.
#[tauri::command]
pub fn load_live(app: State<'_, App>, fixtures: Option<Vec<u32>>) -> Result<ProgrammerView> {
    app.engine
        .state()
        .load_live(fixtures.as_deref(), std::time::Instant::now())?;
    Ok(programmer_view(&app))
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

// --- Stage plan ----------------------------------------------------------

#[tauri::command]
pub fn stage(app: State<'_, App>) -> Stage {
    app.engine.state().stage.clone()
}

/// Sets the stage size and its marks, keeping mover calibration.
#[tauri::command]
pub fn set_stage_plan(app: State<'_, App>, plan: Plan) -> Result<Stage> {
    app.engine.state().stage.set_plan(plan)?;
    app.save()?;
    Ok(app.engine.state().stage.clone())
}

/// Records that `fixture`, where the programmer has it now, is on `mark`.
#[tauri::command]
pub fn calibrate_mark(app: State<'_, App>, fixture: u32, mark: u32) -> Result<Stage> {
    {
        let mut state = app.engine.state();
        let state = &mut *state;
        let patched = state.patch.get(fixture).ok_or(Error::UnknownFixture(fixture))?;
        let options = patched.options;
        let values = state.programmer.values.get(&fixture);
        let (Some(pan), Some(tilt)) = (values.and_then(|v| v.pan), values.and_then(|v| v.tilt)) else {
            return Err(Error::InvalidStage(
                "move the mover onto the mark with the pad first".into(),
            ));
        };
        // Calibration is kept as the fixture sees it, so changing its
        // orientation in the patch later doesn't spoil it.
        let (pan, tilt) = options.orient(pan, tilt);
        let ranges = (f32::from(options.pan_range), f32::from(options.tilt_range));
        state.stage.record(fixture, mark, pan, tilt, ranges)?;
    }
    app.save()?;
    Ok(app.engine.state().stage.clone())
}

/// Sets where a fixture hangs, measured by hand: across, up stage and height,
/// each `None` when not measured.
#[tauri::command]
pub fn set_fixture_position(app: State<'_, App>, fixture: u32, position: [Option<f32>; 3]) -> Result<Stage> {
    app.engine.state().stage.set_position(fixture, position)?;
    app.save()?;
    Ok(app.engine.state().stage.clone())
}

/// Places a fixture on the plan for the Stage page: where it hangs, where it
/// points and how wide its beam is.
#[tauri::command]
pub fn set_placement(app: State<'_, App>, placed: Placed) -> Result<Stage> {
    app.engine.state().stage.set_placement(placed)?;
    app.save()?;
    Ok(app.engine.state().stage.clone())
}

/// Forgets one of a mover's readings, or all of them when `mark` is `None`.
#[tauri::command]
pub fn forget_calibration(app: State<'_, App>, fixture: u32, mark: Option<u32>) -> Result<Stage> {
    app.engine.state().stage.forget(fixture, mark);
    app.save()?;
    Ok(app.engine.state().stage.clone())
}

/// Points every selected, calibrated mover at a spot on the plan, `height`
/// above the floor there (the stage floor on the stage, the studio floor round it).
#[tauri::command]
pub fn point_at(app: State<'_, App>, x: f32, y: f32, height: f32) -> ProgrammerView {
    change_programmer(&app, |state| {
        let z = state.stage.floor_at(x, y) + height;
        let selection = state.programmer.selection.clone();
        for id in selection {
            let (Some(patched), Some(solution)) = (state.patch.get(id), state.stage.solution(id)) else {
                continue;
            };
            let options = patched.options;
            let values = state.programmer.values.entry(id).or_default();
            let current = options.orient(values.pan.unwrap_or(0.5), values.tilt.unwrap_or(0.5));
            let aim = solution.aim([x, y, z], current);
            let (pan, tilt) = options.unorient(aim.pan, aim.tilt);
            values.pan = Some(pan);
            values.tilt = Some(tilt);
        }
    })
}
