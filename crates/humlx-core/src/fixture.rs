//! Fixture profiles: what each DMX channel of a fixture does, per mode.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// What a channel controls. Used to group controls in the UI and, later, to
/// drive the programmer and presets by attribute rather than by raw channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Attribute {
    Intensity,
    Red,
    Green,
    Blue,
    White,
    WarmWhite,
    CoolWhite,
    Amber,
    Lime,
    Uv,
    Cyan,
    Magenta,
    Yellow,
    ColorWheel,
    ColorTemperature,
    Pan,
    Tilt,
    PanTiltSpeed,
    Shutter,
    Gobo,
    GoboRotation,
    Prism,
    Zoom,
    Focus,
    Iris,
    Frost,
    Effect,
    Speed,
    Control,
    Generic,
}

/// Broad groups of attributes, matching how presets are organised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Intensity,
    Colour,
    Position,
    Beam,
    Control,
}

impl Attribute {
    pub fn family(self) -> Family {
        use Attribute::*;
        match self {
            Intensity => Family::Intensity,
            Red | Green | Blue | White | WarmWhite | CoolWhite | Amber | Lime | Uv | Cyan | Magenta | Yellow
            | ColorWheel | ColorTemperature => Family::Colour,
            Pan | Tilt | PanTiltSpeed => Family::Position,
            Shutter | Gobo | GoboRotation | Prism | Zoom | Focus | Iris | Frost => Family::Beam,
            Effect | Speed | Control | Generic => Family::Control,
        }
    }

    /// True for additive or subtractive colour mixing channels.
    pub fn is_colour_mix(self) -> bool {
        use Attribute::*;
        matches!(
            self,
            Red | Green | Blue | White | WarmWhite | CoolWhite | Amber | Lime | Uv | Cyan | Magenta | Yellow
        )
    }
}

/// A labelled DMX range within a channel, such as "Strobe slow to fast".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub from: u8,
    pub to: u8,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub name: String,
    pub attribute: Attribute,
    /// For the fine (16-bit) byte of a parameter, the name of its coarse channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fine_of: Option<String>,
    /// Value the channel rests at when nothing is controlling it.
    #[serde(default)]
    pub default: u8,
    /// Value used to make the fixture visible when locating it. When absent a
    /// sensible value is derived from the attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<Capability>,
}

impl Channel {
    /// The value to output when locating the fixture.
    pub fn highlight_value(&self) -> u8 {
        if let Some(value) = self.highlight {
            return value;
        }
        if self.fine_of.is_some() {
            return 0;
        }
        match self.attribute {
            Attribute::Intensity => 255,
            Attribute::Pan | Attribute::Tilt => 128,
            Attribute::Red | Attribute::Green | Attribute::Blue | Attribute::White => 255,
            Attribute::WarmWhite | Attribute::CoolWhite => 255,
            _ => self.default,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mode {
    pub name: String,
    pub channels: Vec<Channel>,
}

/// A fixture type, such as "Generic RGBW Par", with one or more DMX modes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub manufacturer: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    pub modes: Vec<Mode>,
}

impl Profile {
    /// Stable identifier used by the patch, e.g. `generic/rgbw-par`.
    pub fn id(&self) -> String {
        format!("{}/{}", slug(&self.manufacturer), slug(&self.model))
    }

    pub fn mode(&self, name: &str) -> Option<&Mode> {
        self.modes.iter().find(|mode| mode.name == name)
    }

    /// Parses a profile in HumLX's own JSON format and checks it is usable.
    pub fn from_json(json: &str) -> Result<Self> {
        let profile: Profile = serde_json::from_str(json).map_err(|e| Error::InvalidProfile(e.to_string()))?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<()> {
        if self.manufacturer.trim().is_empty() || self.model.trim().is_empty() {
            return Err(Error::InvalidProfile("manufacturer and model are required".into()));
        }
        if self.modes.is_empty() {
            return Err(Error::InvalidProfile(format!("{} has no modes", self.model)));
        }
        for mode in &self.modes {
            if mode.channels.is_empty() || mode.channels.len() > crate::dmx::UNIVERSE_SIZE {
                return Err(Error::InvalidProfile(format!(
                    "mode \"{}\" must have between 1 and 512 channels",
                    mode.name
                )));
            }
        }
        Ok(())
    }
}

/// Lowercase, hyphen-separated form of a name for use in identifiers.
pub fn slug(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

/// Profiles that ship with HumLX, covering common generic fixture types.
pub fn bundled() -> Vec<Profile> {
    const SOURCES: &[&str] = &[
        include_str!("../fixtures/generic-dimmer.json"),
        include_str!("../fixtures/generic-rgb-par.json"),
        include_str!("../fixtures/generic-rgbw-par.json"),
        include_str!("../fixtures/generic-moving-head-spot.json"),
        include_str!("../fixtures/generic-moving-head-wash.json"),
    ];
    SOURCES
        .iter()
        .map(|json| Profile::from_json(json).expect("bundled profile is valid"))
        .collect()
}

/// The set of profiles available to patch.
#[derive(Debug, Clone, Default)]
pub struct Library {
    profiles: Vec<Profile>,
}

impl Library {
    pub fn with_bundled() -> Self {
        let mut library = Self::default();
        for profile in bundled() {
            library.insert(profile);
        }
        library
    }

    /// Adds a profile, replacing any existing profile with the same id.
    pub fn insert(&mut self, profile: Profile) {
        let id = profile.id();
        self.profiles.retain(|p| p.id() != id);
        self.profiles.push(profile);
        self.profiles
            .sort_by(|a, b| (&a.manufacturer, &a.model).cmp(&(&b.manufacturer, &b.model)));
    }

    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id() == id)
    }

    pub fn mode(&self, id: &str, mode: &str) -> Result<&Mode> {
        let profile = self.get(id).ok_or_else(|| Error::UnknownProfile(id.to_string()))?;
        profile.mode(mode).ok_or_else(|| Error::UnknownMode {
            profile: id.to_string(),
            mode: mode.to_string(),
        })
    }

    pub fn profiles(&self) -> &[Profile] {
        &self.profiles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_profiles_load() {
        let library = Library::with_bundled();
        assert_eq!(library.profiles().len(), 5);
        let par = library.get("generic/rgbw-par").expect("rgbw par");
        assert_eq!(par.mode("4ch RGBW").unwrap().channels.len(), 4);
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Chauvet DJ"), "chauvet-dj");
        assert_eq!(slug("  Rogue R2X  Spot! "), "rogue-r2x-spot");
    }

    #[test]
    fn insert_replaces_same_id() {
        let mut library = Library::with_bundled();
        let mut dimmer = library.get("generic/dimmer").unwrap().clone();
        dimmer.categories = vec!["Replaced".into()];
        library.insert(dimmer);
        assert_eq!(library.profiles().len(), 5);
        assert_eq!(library.get("generic/dimmer").unwrap().categories, vec!["Replaced"]);
    }

    #[test]
    fn highlight_defaults() {
        let channel = |attribute| Channel {
            name: "x".into(),
            attribute,
            fine_of: None,
            default: 3,
            highlight: None,
            capabilities: vec![],
        };
        assert_eq!(channel(Attribute::Intensity).highlight_value(), 255);
        assert_eq!(channel(Attribute::Pan).highlight_value(), 128);
        assert_eq!(channel(Attribute::Gobo).highlight_value(), 3);
    }
}
