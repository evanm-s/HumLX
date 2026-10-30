//! Saving and loading the show: the patch, programming and settings.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine::OutputSettings;
use crate::error::{Error, Result};
use crate::fixture::Profile;
use crate::osc::OscSettings;
use crate::patch::Patch;
use crate::playback::{CueList, Effect};
use crate::programmer::{Group, Preset, Rgb};

const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Show {
    pub version: u32,
    #[serde(default)]
    pub patch: Patch,
    #[serde(default)]
    pub output: OutputSettings,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Group>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<Preset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cue_lists: Vec<CueList>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub osc: OscSettings,
    /// Colour that Highlight turns the selected fixtures.
    #[serde(default)]
    pub highlight_colour: Rgb,
    /// Fixture types the patch uses that aren't built in, so a show file
    /// opens on another computer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<Profile>,
}

impl Default for Show {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            patch: Patch::default(),
            output: OutputSettings::default(),
            groups: Vec::new(),
            presets: Vec::new(),
            cue_lists: Vec::new(),
            effects: Vec::new(),
            osc: OscSettings::default(),
            highlight_colour: Rgb::WHITE,
            profiles: Vec::new(),
        }
    }
}

impl Show {
    pub fn from_json(json: &str) -> Result<Self> {
        let show: Show = serde_json::from_str(json).map_err(|e| Error::InvalidShow(e.to_string()))?;
        if show.version > FORMAT_VERSION {
            return Err(Error::InvalidShow(format!(
                "this show was saved by a newer version of HumLX (format {})",
                show.version
            )));
        }
        Ok(show)
    }

    pub fn load(path: &Path) -> Result<Self> {
        Self::from_json(&std::fs::read_to_string(path)?)
    }

    /// Writes the show atomically, so a crash mid-save never leaves a broken file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(|e| Error::InvalidShow(e.to_string()))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, json)?;
        std::fs::rename(&temp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Library;
    use crate::patch::NewFixtures;

    #[test]
    fn round_trips_through_disk() {
        let library = Library::with_bundled();
        let mut show = Show::default();
        show.patch
            .add(
                &library,
                &NewFixtures {
                    profile: "generic/dimmer".into(),
                    mode: "1ch".into(),
                    count: 4,
                    first_id: None,
                    address: None,
                    name: Some("House".into()),
                },
            )
            .unwrap();
        show.output.target = "192.168.1.50".parse().unwrap();
        show.groups.push(crate::programmer::Group {
            id: 1,
            name: "House".into(),
            fixtures: vec![1, 2, 3, 4],
        });
        let mut programmer = crate::programmer::Programmer::default();
        programmer.select(&[1, 2], crate::programmer::SelectMode::Set);
        programmer.update(|v| v.intensity = Some(0.5));
        show.cue_lists.push(crate::playback::CueList {
            id: 1,
            name: "Main".into(),
            cues: vec![crate::playback::Cue {
                name: "Preset".into(),
                fade: 3.0,
                values: programmer.values.clone(),
            }],
        });
        show.effects.push(crate::playback::Effect {
            id: 1,
            name: "Rainbow".into(),
            kind: crate::playback::EffectKind::Rainbow,
            wave: crate::playback::Wave::Sine,
            fixtures: vec![1, 2],
            rate: 0.2,
            size: 1.0,
            spread: 0.5,
        });
        show.presets.push(
            crate::programmer::Preset::record(1, "Half".into(), crate::fixture::Family::Intensity, &programmer)
                .unwrap(),
        );

        let dir = std::env::temp_dir().join(format!("humlx-show-test-{}", std::process::id()));
        let path = dir.join("show.json");
        show.save(&path).unwrap();
        assert_eq!(Show::load(&path).unwrap(), show);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_newer_format() {
        assert!(Show::from_json(r#"{"version": 99}"#).is_err());
        assert_eq!(Show::from_json(r#"{"version": 1}"#).unwrap(), Show::default());
    }
}
