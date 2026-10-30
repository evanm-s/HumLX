//! Commands for cue lists, their playback and effects.

use std::time::Instant;

use humlx_core::engine::State as EngineState;
use humlx_core::playback::{Cue, CueList, CueMode, Effect, EffectKind, Wave};
use humlx_core::programmer::next_id;
use humlx_core::{Error, Result};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::App;

/// Live state of one running cue list, polled by the UI.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStatus {
    list: u32,
    current: usize,
    fading: bool,
    /// Seconds until the next cue follows, when the current cue is a follow cue.
    follow_in: Option<f32>,
}

pub fn statuses(state: &EngineState) -> Vec<PlaybackStatus> {
    let now = Instant::now();
    state
        .playbacks
        .running()
        .map(|(list, playback)| PlaybackStatus {
            list: *list,
            current: playback.current,
            fading: playback.fading(now),
            follow_in: playback.follow_in(now),
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackView {
    cue_lists: Vec<CueList>,
    effects: Vec<Effect>,
    /// Fader level of each cue list, by id.
    masters: Vec<(u32, f32)>,
}

fn view(app: &App) -> PlaybackView {
    let state = app.engine.state();
    PlaybackView {
        cue_lists: state.cue_lists.clone(),
        effects: state.effects.clone(),
        masters: masters(&state),
    }
}

pub fn masters(state: &EngineState) -> Vec<(u32, f32)> {
    state
        .cue_lists
        .iter()
        .map(|l| (l.id, state.playbacks.master(l.id)))
        .collect()
}

/// Runs a change, saves the show if `save`, and returns the new playback state.
fn change(app: &App, save: bool, f: impl FnOnce(&mut EngineState) -> Result<()>) -> Result<PlaybackView> {
    f(&mut app.engine.state())?;
    if save {
        app.save()?;
    }
    Ok(view(app))
}

fn list_mut(state: &mut EngineState, id: u32) -> Result<&mut CueList> {
    state
        .cue_lists
        .iter_mut()
        .find(|l| l.id == id)
        .ok_or(Error::UnknownCueList(id))
}

/// Restarts a running list on its current cue after its cues changed.
fn refresh(state: &mut EngineState, id: u32) {
    if let Some(list) = state.cue_lists.iter().find(|l| l.id == id) {
        state.playbacks.refresh(list, Instant::now());
    }
}

#[tauri::command]
pub fn playback(app: State<'_, App>) -> PlaybackView {
    view(&app)
}

#[tauri::command]
pub fn new_cue_list(app: State<'_, App>, name: String) -> Result<PlaybackView> {
    change(&app, true, |state| {
        add_cue_list(state, &name);
        Ok(())
    })
}

/// Adds an empty cue list, named "Cue list N" when the name is blank, and returns its id.
fn add_cue_list(state: &mut EngineState, name: &str) -> u32 {
    let id = next_id(state.cue_lists.iter().map(|l| l.id));
    let name = match name.trim() {
        "" => format!("Cue list {id}"),
        n => n.to_string(),
    };
    state.cue_lists.push(CueList { id, name, cues: vec![] });
    id
}

#[tauri::command]
pub fn rename_cue_list(app: State<'_, App>, id: u32, name: String) -> Result<PlaybackView> {
    change(&app, true, |state| {
        list_mut(state, id)?.name = name.trim().to_string();
        Ok(())
    })
}

#[tauri::command]
pub fn delete_cue_list(app: State<'_, App>, id: u32) -> Result<PlaybackView> {
    change(&app, true, |state| {
        state.cue_lists.retain(|l| l.id != id);
        state.playbacks.release(id);
        Ok(())
    })
}

/// Where a recorded cue goes: an existing cue list, or a new one with this name.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecordTarget {
    List(u32),
    New(String),
}

/// Records the programmer as a new cue, after cue `after` (0-based) or at the
/// end, of an existing cue list or a new one. The programmer is then cleared and the list goes to the new cue, so
/// what was on stage stays on stage.
#[tauri::command]
pub fn record_cue(
    app: State<'_, App>,
    target: RecordTarget,
    name: String,
    fade: f32,
    after: Option<usize>,
) -> Result<PlaybackView> {
    change(&app, true, |state| {
        if state.programmer.values.is_empty() {
            return Err(Error::NothingToRecord);
        }
        let values = state.programmer.values.clone();
        let list = match target {
            RecordTarget::List(id) => id,
            RecordTarget::New(list_name) => add_cue_list(state, &list_name),
        };
        let cue_list = list_mut(state, list)?;
        let index = after.map_or(cue_list.cues.len(), |i| (i + 1).min(cue_list.cues.len()));
        let name = match name.trim() {
            "" => format!("Cue {}", index + 1),
            n => n.to_string(),
        };
        cue_list.cues.insert(
            index,
            Cue {
                name,
                fade: fade.max(0.0),
                values,
                ..Default::default()
            },
        );
        let snapshot = cue_list.clone();
        state.programmer.clear(None);
        state.playbacks.go_to(&snapshot, index, Instant::now())?;
        state.playbacks.refresh(&snapshot, Instant::now());
        Ok(())
    })
}

/// Adds the programmer's values to an existing cue.
#[tauri::command]
pub fn update_cue(app: State<'_, App>, list: u32, index: usize) -> Result<PlaybackView> {
    change(&app, true, |state| {
        if state.programmer.values.is_empty() {
            return Err(Error::NothingToRecord);
        }
        let values = state.programmer.values.clone();
        let cue = list_mut(state, list)?
            .cues
            .get_mut(index)
            .ok_or(Error::UnknownCue(index + 1))?;
        for (fixture, v) in values {
            cue.values.entry(fixture).or_default().merge(&v);
        }
        state.programmer.clear(None);
        refresh(state, list);
        Ok(())
    })
}

#[tauri::command]
pub fn edit_cue(
    app: State<'_, App>,
    list: u32,
    index: usize,
    name: Option<String>,
    fade: Option<f32>,
    mode: Option<CueMode>,
    follow: Option<f32>,
) -> Result<PlaybackView> {
    change(&app, true, |state| {
        let cue = list_mut(state, list)?
            .cues
            .get_mut(index)
            .ok_or(Error::UnknownCue(index + 1))?;
        if let Some(name) = name {
            cue.name = name.trim().to_string();
        }
        if let Some(fade) = fade {
            cue.fade = fade.clamp(0.0, 600.0);
        }
        let timing = mode.is_some() || follow.is_some();
        if let Some(mode) = mode {
            cue.mode = mode;
        }
        if let Some(follow) = follow {
            cue.follow = follow.clamp(0.0, 3600.0);
        }
        // A running follow picks up the new time straight away.
        if timing {
            refresh(state, list);
        }
        Ok(())
    })
}

#[tauri::command]
pub fn delete_cue(app: State<'_, App>, list: u32, index: usize) -> Result<PlaybackView> {
    change(&app, true, |state| {
        let cues = &mut list_mut(state, list)?.cues;
        if index >= cues.len() {
            return Err(Error::UnknownCue(index + 1));
        }
        cues.remove(index);
        refresh(state, list);
        Ok(())
    })
}

fn with_list(app: &App, list: u32, f: impl FnOnce(&mut EngineState, &CueList) -> Result<()>) -> Result<PlaybackView> {
    change(app, false, |state| {
        let cue_list = state
            .cue_lists
            .iter()
            .find(|l| l.id == list)
            .cloned()
            .ok_or(Error::UnknownCueList(list))?;
        f(state, &cue_list)
    })
}

#[tauri::command]
pub fn go(app: State<'_, App>, list: u32) -> Result<PlaybackView> {
    with_list(&app, list, |state, l| state.playbacks.go(l, Instant::now()))
}

#[tauri::command]
pub fn back(app: State<'_, App>, list: u32) -> Result<PlaybackView> {
    with_list(&app, list, |state, l| state.playbacks.back(l, Instant::now()))
}

#[tauri::command]
pub fn go_to(app: State<'_, App>, list: u32, index: usize) -> Result<PlaybackView> {
    with_list(&app, list, |state, l| state.playbacks.go_to(l, index, Instant::now()))
}

#[tauri::command]
pub fn release_cue_list(app: State<'_, App>, list: u32) -> Result<PlaybackView> {
    change(&app, false, |state| {
        state.playbacks.release(list);
        Ok(())
    })
}

#[tauri::command]
pub fn release_all(app: State<'_, App>) -> Result<PlaybackView> {
    change(&app, false, |state| {
        state.playbacks.release_all();
        state.running_effects.stop_all();
        Ok(())
    })
}

#[tauri::command]
pub fn set_master(app: State<'_, App>, list: u32, level: f32) -> Result<PlaybackView> {
    change(&app, false, |state| {
        state.playbacks.set_master(list, level);
        Ok(())
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectSettings {
    name: String,
    kind: EffectKind,
    wave: Wave,
    rate: f32,
    size: f32,
    spread: f32,
}

/// Creates an effect on the selected fixtures, or changes effect `id`'s
/// settings (keeping its fixtures). A new effect starts running.
#[tauri::command]
pub fn save_effect(app: State<'_, App>, settings: EffectSettings, id: Option<u32>) -> Result<PlaybackView> {
    change(&app, true, |state| {
        let EffectSettings {
            name,
            kind,
            wave,
            rate,
            size,
            spread,
        } = settings;
        let (rate, size, spread) = (rate.clamp(0.01, 20.0), size.clamp(0.0, 1.0), spread.clamp(0.0, 1.0));
        match id {
            Some(id) => {
                let effect = state
                    .effects
                    .iter_mut()
                    .find(|e| e.id == id)
                    .ok_or(Error::UnknownEffect(id))?;
                effect.name = name.trim().to_string();
                effect.kind = kind;
                effect.wave = wave;
                effect.rate = rate;
                effect.size = size;
                effect.spread = spread;
            }
            None => {
                if state.programmer.selection.is_empty() {
                    return Err(Error::InvalidShow("select the fixtures for the effect first".into()));
                }
                let id = next_id(state.effects.iter().map(|e| e.id));
                state.effects.push(Effect {
                    id,
                    name: name.trim().to_string(),
                    kind,
                    wave,
                    fixtures: state.programmer.selection.clone(),
                    rate,
                    size,
                    spread,
                });
                state.effects.sort_by_key(|e| e.id);
                state.running_effects.start(id, Instant::now());
            }
        }
        Ok(())
    })
}

#[tauri::command]
pub fn delete_effect(app: State<'_, App>, id: u32) -> Result<PlaybackView> {
    change(&app, true, |state| {
        state.effects.retain(|e| e.id != id);
        state.running_effects.stop(id);
        Ok(())
    })
}

#[tauri::command]
pub fn set_effect_running(app: State<'_, App>, id: u32, on: bool) -> Result<PlaybackView> {
    change(&app, false, |state| {
        if !state.effects.iter().any(|e| e.id == id) {
            return Err(Error::UnknownEffect(id));
        }
        if on {
            state.running_effects.start(id, Instant::now());
        } else {
            state.running_effects.stop(id);
        }
        Ok(())
    })
}
