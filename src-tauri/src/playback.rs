//! Commands for cue lists, their playback and effects.

use std::time::Instant;

use humlx_core::effect::{lay_over, Direction, Effect, EffectSize, Layering, Pattern, Shape, Speed};
use humlx_core::engine::State as EngineState;
use humlx_core::layout::Layout;
use humlx_core::playback::{Cue, CueList, CueMode};
use humlx_core::programmer::next_id;
use humlx_core::programmer::{Rgb, Values};
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
    /// The master that scales every effect, 0 to 1.
    effect_size: f32,
    /// Tempo of speed masters A and B.
    speed_bpm: [f32; 2],
    /// Fader level of each cue list, by id.
    masters: Vec<(u32, f32)>,
    /// The button grid, the fader slots and the selected list.
    layout: Layout,
}

fn view(app: &App) -> PlaybackView {
    let state = app.engine.state();
    PlaybackView {
        cue_lists: state.cue_lists.clone(),
        effect_size: state.effect_size.0,
        speed_bpm: state.speed_masters.bpms(),
        masters: masters(&state),
        layout: state.layout.clone(),
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
        state.layout.selected = Some(add_cue_list(state, &name));
        Ok(())
    })
}

/// Copies a cue list, with all its cues, to a new number, and selects the copy.
#[tauri::command]
pub fn duplicate_cue_list(app: State<'_, App>, id: u32) -> Result<PlaybackView> {
    change(&app, true, |state| {
        let mut copy = list_mut(state, id)?.clone();
        copy.id = next_id(state.cue_lists.iter().map(|l| l.id));
        copy.name = format!("{} copy", copy.name);
        state.layout.selected = Some(copy.id);
        state.cue_lists.push(copy);
        Ok(())
    })
}

/// Makes `list` the selected cue list, which GO, Back and Record act on.
#[tauri::command]
pub fn select_cue_list(app: State<'_, App>, list: Option<u32>) -> Result<PlaybackView> {
    change(&app, true, |state| {
        if let Some(id) = list {
            list_mut(state, id)?;
        }
        state.layout.selected = list;
        Ok(())
    })
}

fn no_button() -> Error {
    Error::Unsupported("no such grid button".into())
}

/// Puts `list` on grid button `button` (from 0) of page `page`, or empties it.
#[tauri::command]
pub fn set_grid_button(app: State<'_, App>, page: usize, button: usize, list: Option<u32>) -> Result<PlaybackView> {
    change(&app, true, |state| {
        if let Some(id) = list {
            list_mut(state, id)?;
        }
        *state.layout.button_mut(page, button).ok_or_else(no_button)? = list;
        Ok(())
    })
}

/// Swaps two grid buttons, given as (page, button).
#[tauri::command]
pub fn swap_grid_buttons(app: State<'_, App>, a: (usize, usize), b: (usize, usize)) -> Result<PlaybackView> {
    change(&app, true, |state| {
        if state.layout.swap_buttons(a, b) {
            Ok(())
        } else {
            Err(no_button())
        }
    })
}

#[tauri::command]
pub fn add_grid_page(app: State<'_, App>, name: String) -> Result<PlaybackView> {
    change(&app, true, |state| {
        state.layout.add_page(&name);
        Ok(())
    })
}

#[tauri::command]
pub fn rename_grid_page(app: State<'_, App>, page: usize, name: String) -> Result<PlaybackView> {
    change(&app, true, |state| {
        let page = state
            .layout
            .pages
            .get_mut(page)
            .ok_or(Error::Unsupported("no such grid page".into()))?;
        page.name = name.trim().to_string();
        Ok(())
    })
}

/// Removes a grid page; its lists stay on the Cue Lists screen.
#[tauri::command]
pub fn delete_grid_page(app: State<'_, App>, page: usize) -> Result<PlaybackView> {
    change(&app, true, |state| {
        if state.layout.remove_page(page) {
            Ok(())
        } else {
            Err(Error::Unsupported("the only grid page can't be removed".into()))
        }
    })
}

/// Loads `list` into fader slot `slot` (from 0), or empties the slot.
#[tauri::command]
pub fn set_slot(app: State<'_, App>, slot: usize, list: Option<u32>) -> Result<PlaybackView> {
    change(&app, true, |state| {
        if let Some(id) = list {
            list_mut(state, id)?;
        }
        let place = state
            .layout
            .slots
            .get_mut(slot)
            .ok_or(Error::Unsupported("no such fader slot".into()))?;
        *place = list;
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
        state.layout.forget(id);
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
    /// A new one-cue list (a look), named `name` or "Look N", placed on a grid button.
    Button {
        page: usize,
        button: usize,
        name: String,
    },
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
        if state.programmer.values.is_empty() && state.programmer.effects.is_empty() {
            return Err(Error::NothingToRecord);
        }
        let values = state.programmer.values.clone();
        let effects = state.programmer.effects.clone();
        let list = match target {
            RecordTarget::List(id) => id,
            // A new list goes into the first free fader slot, so it can be
            // seen and run on the Playback screen straight away.
            RecordTarget::New(list_name) => {
                let id = add_cue_list(state, &list_name);
                if let Some(slot) = state.layout.free_slot() {
                    state.layout.slots[slot] = Some(id);
                }
                id
            }
            RecordTarget::Button { page, button, name } => {
                state.layout.button_mut(page, button).ok_or_else(no_button)?;
                let id = add_cue_list(state, &name);
                if name.trim().is_empty() {
                    list_mut(state, id)?.name = format!("Look {id}");
                }
                *state.layout.button_mut(page, button).ok_or_else(no_button)? = Some(id);
                id
            }
        };
        state.layout.selected = Some(list);
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
                effects,
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
        if state.programmer.values.is_empty() && state.programmer.effects.is_empty() {
            return Err(Error::NothingToRecord);
        }
        let values = state.programmer.values.clone();
        let effects = state.programmer.effects.clone();
        let cue = list_mut(state, list)?
            .cues
            .get_mut(index)
            .ok_or(Error::UnknownCue(index + 1))?;
        for (fixture, v) in values {
            cue.values.entry(fixture).or_default().merge(&v);
        }
        for effect in effects {
            lay_over(&mut cue.effects, effect);
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
    shape: Shape,
    rate: f32,
    size: f32,
    spread: f32,
    #[serde(default)]
    pattern: Pattern,
    #[serde(default)]
    direction: Direction,
    #[serde(default = "full")]
    width: f32,
    #[serde(default)]
    speed: Speed,
    #[serde(default = "full")]
    multiplier: f32,
    /// Fade between steps; the shape's own when not given.
    #[serde(default)]
    transition: Option<f32>,
    /// The colours a two-colour swap swaps between, or colour steps step through.
    #[serde(default)]
    colours: Vec<Rgb>,
    /// Steps set by hand in the step editor; the shape's own when not given.
    #[serde(default)]
    steps: Option<Vec<Values>>,
    /// Whether the effect works from the look underneath; the shape's own when not given.
    #[serde(default)]
    layering: Option<Layering>,
}

fn full() -> f32 {
    1.0
}

impl EffectSettings {
    /// Sets `effect` up from these settings, remaking its steps from the shape.
    fn apply_to(self, effect: &mut Effect) {
        effect.reshape(self.shape, &self.colours);
        effect.name = self.name.trim().to_string();
        effect.rate = self.rate.clamp(0.01, 20.0);
        effect.size = self.size.clamp(0.0, 1.0);
        effect.spread = self.spread.clamp(0.0, 1.0);
        effect.pattern = self.pattern;
        effect.direction = self.direction;
        effect.width = self.width.clamp(0.05, 1.0);
        effect.speed = self.speed;
        effect.multiplier = self.multiplier.clamp(0.125, 8.0);
        if let Some(t) = self.transition {
            effect.transition = t.clamp(0.0, 1.0);
        }
        if let Some(steps) = self.steps {
            effect.set_steps(steps);
        }
        if let Some(layering) = self.layering {
            effect.layering = layering;
        }
    }
}

/// Creates an effect in the programmer on the selected fixtures, or changes
/// programmer effect `id`'s settings (keeping its fixtures). Programmer
/// effects run straight away and record into cues with the values.
#[tauri::command]
pub fn save_effect(app: State<'_, App>, settings: EffectSettings, id: Option<u32>) -> Result<PlaybackView> {
    change(&app, false, |state| {
        match id {
            Some(id) => {
                let effect = state
                    .programmer
                    .effects
                    .iter_mut()
                    .find(|e| e.id == id)
                    .ok_or(Error::UnknownEffect(id))?;
                settings.apply_to(effect);
            }
            None => {
                if state.programmer.selection.is_empty() {
                    return Err(Error::InvalidShow("select the fixtures for the effect first".into()));
                }
                let id = next_id(state.programmer.effects.iter().map(|e| e.id));
                let mut effect = Effect::new(
                    id,
                    String::new(),
                    settings.shape,
                    &[],
                    state.programmer.selection.clone(),
                );
                settings.apply_to(&mut effect);
                state.programmer.effects.push(effect);
                state.running_effects.start(id, Instant::now());
            }
        }
        Ok(())
    })
}

#[tauri::command]
pub fn delete_effect(app: State<'_, App>, id: u32) -> Result<PlaybackView> {
    change(&app, false, |state| {
        state.programmer.effects.retain(|e| e.id != id);
        state.running_effects.stop(id);
        Ok(())
    })
}

/// A tap of speed master 0 (A) or 1 (B), which sets its tempo after two taps.
#[tauri::command]
pub fn tap_speed(app: State<'_, App>, master: usize) -> Result<PlaybackView> {
    change(&app, true, |state| {
        let m = state
            .speed_masters
            .0
            .get_mut(master)
            .ok_or(Error::Unsupported("no such speed master".into()))?;
        m.tap(Instant::now());
        Ok(())
    })
}

/// Sets speed master 0 (A) or 1 (B) to `bpm`.
#[tauri::command]
pub fn set_speed_bpm(app: State<'_, App>, master: usize, bpm: f32) -> Result<PlaybackView> {
    change(&app, true, |state| {
        let m = state
            .speed_masters
            .0
            .get_mut(master)
            .ok_or(Error::Unsupported("no such speed master".into()))?;
        m.set_bpm(bpm, Instant::now());
        Ok(())
    })
}

/// Sets the master that scales every effect, from 0 to 1.
#[tauri::command]
pub fn set_effect_size(app: State<'_, App>, level: f32) -> Result<PlaybackView> {
    change(&app, false, |state| {
        state.effect_size = EffectSize(level.clamp(0.0, 1.0));
        Ok(())
    })
}
