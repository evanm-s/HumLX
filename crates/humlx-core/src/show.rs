//! Saving and loading the show: the patch and output settings.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine::OutputSettings;
use crate::error::{Error, Result};
use crate::patch::Patch;

const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Show {
    pub version: u32,
    #[serde(default)]
    pub patch: Patch,
    #[serde(default)]
    pub output: OutputSettings,
}

impl Default for Show {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            patch: Patch::default(),
            output: OutputSettings::default(),
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
