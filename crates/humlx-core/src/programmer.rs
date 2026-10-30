//! The programmer: which fixtures are selected and the values being set on
//! them, plus the groups and presets recorded from it.
//!
//! Values are kept in fixture-independent terms (an intensity, an RGB colour,
//! a pan and tilt) and only turned into DMX for each fixture's own channels
//! when the universe is composed. That is what lets one colour preset work on
//! an RGBW par, a CMY spot and a colour-wheel mover alike.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::fixture::{Attribute, Channel, Family, Mode};

/// A colour with each component from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Rgb {
    pub const WHITE: Rgb = Rgb { r: 1.0, g: 1.0, b: 1.0 };

    fn clamped(self) -> Self {
        Self {
            r: self.r.clamp(0.0, 1.0),
            g: self.g.clamp(0.0, 1.0),
            b: self.b.clamp(0.0, 1.0),
        }
    }

    fn distance(self, other: Rgb) -> f32 {
        (self.r - other.r).powi(2) + (self.g - other.g).powi(2) + (self.b - other.b).powi(2)
    }
}

/// What the programmer (or a preset) sets on one fixture. `None` and missing
/// entries leave the fixture's channels at their resting level.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Values {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intensity: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<Rgb>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pan: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tilt: Option<f32>,
    /// Any other attribute set directly, such as zoom or gobo, from 0 to 1.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub other: BTreeMap<Attribute, f32>,
}

impl Values {
    pub fn is_empty(&self) -> bool {
        self.intensity.is_none()
            && self.colour.is_none()
            && self.pan.is_none()
            && self.tilt.is_none()
            && self.other.is_empty()
    }

    /// Only the values belonging to `family`.
    pub fn only(&self, family: Family) -> Values {
        Values {
            intensity: if family == Family::Intensity {
                self.intensity
            } else {
                None
            },
            colour: if family == Family::Colour { self.colour } else { None },
            pan: if family == Family::Position { self.pan } else { None },
            tilt: if family == Family::Position { self.tilt } else { None },
            other: self
                .other
                .iter()
                .filter(|(attribute, _)| attribute.family() == family)
                .map(|(a, v)| (*a, *v))
                .collect(),
        }
    }

    /// Removes the values belonging to `family`.
    pub fn clear(&mut self, family: Family) {
        match family {
            Family::Intensity => self.intensity = None,
            Family::Colour => self.colour = None,
            Family::Position => {
                self.pan = None;
                self.tilt = None;
            }
            Family::Beam | Family::Control => {}
        }
        self.other.retain(|attribute, _| attribute.family() != family);
    }

    /// Overlays every value set in `other` onto this one.
    pub fn merge(&mut self, other: &Values) {
        self.intensity = other.intensity.or(self.intensity);
        self.colour = other.colour.or(self.colour);
        self.pan = other.pan.or(self.pan);
        self.tilt = other.tilt.or(self.tilt);
        self.other.extend(other.other.iter().map(|(a, v)| (*a, *v)));
    }

    /// Works out the DMX value of each of `mode`'s channels. `None` means the
    /// channel is not controlled by these values.
    pub fn render(&self, mode: &Mode) -> Vec<Option<u8>> {
        let channels = &mode.channels;
        let mut out = vec![None; channels.len()];
        let has = |attribute: Attribute| coarse(channels, attribute).next().is_some();
        let has_any = |attributes: &[Attribute]| attributes.iter().any(|a| has(*a));
        let set = |out: &mut Vec<Option<u8>>, attribute: Attribute, value: f32| {
            for i in coarse(channels, attribute) {
                write_16_bit(channels, out, i, value);
            }
        };

        let has_dimmer = has(Attribute::Intensity);
        if let Some(level) = self.intensity {
            set(&mut out, Attribute::Intensity, level);
        }

        // Without a dimmer, the colour mix doubles as the intensity.
        let mix = if has_dimmer {
            self.colour
        } else if self.colour.is_some() || self.intensity.is_some() {
            let colour = self.colour.unwrap_or(Rgb::WHITE);
            let level = self.intensity.unwrap_or(0.0).clamp(0.0, 1.0);
            Some(Rgb {
                r: colour.r * level,
                g: colour.g * level,
                b: colour.b * level,
            })
        } else {
            None
        };

        const WHITES: [Attribute; 3] = [Attribute::White, Attribute::WarmWhite, Attribute::CoolWhite];
        if let Some(colour) = mix.map(Rgb::clamped) {
            if has_any(&[Attribute::Red, Attribute::Green, Attribute::Blue]) {
                // Take the shared part of the colour from the white emitter.
                let white = if has_any(&WHITES) {
                    colour.r.min(colour.g).min(colour.b)
                } else {
                    0.0
                };
                set(&mut out, Attribute::Red, colour.r - white);
                set(&mut out, Attribute::Green, colour.g - white);
                set(&mut out, Attribute::Blue, colour.b - white);
                for attribute in WHITES {
                    set(&mut out, attribute, white);
                }
                for attribute in [Attribute::Amber, Attribute::Lime, Attribute::Uv] {
                    set(&mut out, attribute, 0.0);
                }
            } else if has_any(&[Attribute::Cyan, Attribute::Magenta, Attribute::Yellow]) {
                set(&mut out, Attribute::Cyan, 1.0 - colour.r);
                set(&mut out, Attribute::Magenta, 1.0 - colour.g);
                set(&mut out, Attribute::Yellow, 1.0 - colour.b);
            } else {
                if !has_dimmer {
                    // A white-only fixture without a dimmer: its white is its intensity.
                    let level = colour.r.max(colour.g).max(colour.b);
                    for attribute in WHITES {
                        set(&mut out, attribute, level);
                    }
                }
                if let Some(target) = self.colour {
                    for i in coarse(channels, Attribute::ColorWheel) {
                        if let Some(value) = nearest_wheel_slot(&channels[i], target) {
                            out[i] = Some(value);
                        }
                    }
                }
            }
        }

        // A bi-colour fixture with separate warm and cool emitters has no white
        // balance channel, so the balance (0 warm, 1 cool) cross-fades the two,
        // keeping both at full through the middle.
        if let Some(&balance) = self.other.get(&Attribute::ColorTemperature) {
            let bi_colour = has(Attribute::WarmWhite)
                && has(Attribute::CoolWhite)
                && !has(Attribute::ColorTemperature)
                && !has_any(&[Attribute::Red, Attribute::Green, Attribute::Blue]);
            if bi_colour {
                let level = if has_dimmer {
                    1.0
                } else {
                    mix.map(Rgb::clamped).map_or(0.0, |c| c.r.max(c.g).max(c.b))
                };
                let balance = balance.clamp(0.0, 1.0);
                set(&mut out, Attribute::WarmWhite, level * (2.0 * (1.0 - balance)).min(1.0));
                set(&mut out, Attribute::CoolWhite, level * (2.0 * balance).min(1.0));
            }
        }

        if let Some(pan) = self.pan {
            set(&mut out, Attribute::Pan, pan);
        }
        if let Some(tilt) = self.tilt {
            set(&mut out, Attribute::Tilt, tilt);
        }
        for (attribute, value) in &self.other {
            set(&mut out, *attribute, *value);
        }
        out
    }
}

/// Indices of the coarse channels controlling `attribute`.
fn coarse(channels: &[Channel], attribute: Attribute) -> impl Iterator<Item = usize> + '_ {
    channels
        .iter()
        .enumerate()
        .filter(move |(_, c)| c.attribute == attribute && c.fine_of.is_none())
        .map(|(i, _)| i)
}

/// Writes a 0..1 value to a coarse channel and, if the mode has one, its fine channel.
fn write_16_bit(channels: &[Channel], out: &mut [Option<u8>], index: usize, value: f32) {
    let wide = (value.clamp(0.0, 1.0) * 65535.0).round() as u16;
    out[index] = Some((wide >> 8) as u8);
    let name = &channels[index].name;
    for (i, channel) in channels.iter().enumerate() {
        if channel.fine_of.as_deref() == Some(name) {
            out[i] = Some((wide & 0xFF) as u8);
        }
    }
}

/// Picks the colour wheel slot whose label names the colour closest to `target`.
fn nearest_wheel_slot(channel: &Channel, target: Rgb) -> Option<u8> {
    const NAMED: &[(&str, Rgb)] = &[
        ("red", Rgb { r: 1.0, g: 0.0, b: 0.0 }),
        ("orange", Rgb { r: 1.0, g: 0.5, b: 0.0 }),
        (
            "amber",
            Rgb {
                r: 1.0,
                g: 0.75,
                b: 0.0,
            },
        ),
        ("yellow", Rgb { r: 1.0, g: 1.0, b: 0.0 }),
        ("lime", Rgb { r: 0.6, g: 1.0, b: 0.0 }),
        ("green", Rgb { r: 0.0, g: 1.0, b: 0.0 }),
        ("cyan", Rgb { r: 0.0, g: 1.0, b: 1.0 }),
        ("light blue", Rgb { r: 0.5, g: 0.7, b: 1.0 }),
        ("blue", Rgb { r: 0.0, g: 0.0, b: 1.0 }),
        ("uv", Rgb { r: 0.4, g: 0.0, b: 1.0 }),
        ("purple", Rgb { r: 0.6, g: 0.0, b: 1.0 }),
        ("magenta", Rgb { r: 1.0, g: 0.0, b: 1.0 }),
        ("pink", Rgb { r: 1.0, g: 0.4, b: 0.7 }),
        ("open", Rgb::WHITE),
        ("white", Rgb::WHITE),
    ];
    channel
        .capabilities
        .iter()
        .filter_map(|capability| {
            let label = capability.label.to_lowercase();
            // Skip ranges like "Rainbow" or "Red to blue scroll" that are not a single slot.
            if label.contains(" to ") || label.contains("rainbow") || label.contains("scroll") {
                return None;
            }
            let (_, colour) = NAMED.iter().find(|(name, _)| label.contains(name))?;
            Some((colour.distance(target), capability))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, capability)| ((u16::from(capability.from) + u16::from(capability.to)) / 2) as u8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectMode {
    /// Replace the selection.
    Set,
    /// Add fixtures not yet selected and remove those that are.
    Toggle,
    /// Add to the selection, keeping its order.
    Add,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Programmer {
    /// Selected fixtures, in the order they were picked.
    pub selection: Vec<u32>,
    /// Values set on each fixture, including fixtures no longer selected.
    pub values: BTreeMap<u32, Values>,
}

impl Programmer {
    pub fn select(&mut self, fixtures: &[u32], mode: SelectMode) {
        match mode {
            SelectMode::Set => {
                self.selection.clear();
                self.add(fixtures);
            }
            SelectMode::Add => self.add(fixtures),
            SelectMode::Toggle => {
                for id in fixtures {
                    if let Some(index) = self.selection.iter().position(|s| s == id) {
                        self.selection.remove(index);
                    } else {
                        self.selection.push(*id);
                    }
                }
            }
        }
    }

    fn add(&mut self, fixtures: &[u32]) {
        for id in fixtures {
            if !self.selection.contains(id) {
                self.selection.push(*id);
            }
        }
    }

    /// Changes the values of every selected fixture.
    pub fn update(&mut self, change: impl Fn(&mut Values)) {
        for id in &self.selection {
            change(self.values.entry(*id).or_default());
        }
    }

    /// Clears one family from every fixture, or everything when `None`.
    pub fn clear(&mut self, family: Option<Family>) {
        match family {
            None => self.values.clear(),
            Some(family) => {
                for values in self.values.values_mut() {
                    values.clear(family);
                }
                self.values.retain(|_, v| !v.is_empty());
            }
        }
    }

    /// Drops fixtures that are no longer patched.
    pub fn retain(&mut self, patched: impl Fn(u32) -> bool) {
        self.selection.retain(|id| patched(*id));
        self.values.retain(|id, _| patched(*id));
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub id: u32,
    pub name: String,
    pub fixtures: Vec<u32>,
}

/// Stored values for one family, recalled onto the selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: u32,
    pub name: String,
    pub family: Family,
    /// Values for any fixture, used when every recorded fixture had the same.
    #[serde(default, skip_serializing_if = "Values::is_empty")]
    pub all: Values,
    /// Values recorded per fixture, for presets such as positions that differ.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fixtures: BTreeMap<u32, Values>,
}

impl Preset {
    /// Records the programmer's `family` values for the selection, or for every
    /// fixture with such values when nothing is selected.
    pub fn record(id: u32, name: String, family: Family, programmer: &Programmer) -> Result<Self> {
        let recorded: BTreeMap<u32, Values> = programmer
            .values
            .iter()
            .filter(|(fixture, _)| programmer.selection.is_empty() || programmer.selection.contains(fixture))
            .map(|(fixture, values)| (*fixture, values.only(family)))
            .filter(|(_, values)| !values.is_empty())
            .collect();
        let Some(first) = recorded.values().next() else {
            return Err(Error::NothingToRecord);
        };
        let shared = recorded.values().all(|values| values == first);
        Ok(Preset {
            id,
            name,
            family,
            all: if shared { first.clone() } else { Values::default() },
            fixtures: if shared { BTreeMap::new() } else { recorded },
        })
    }

    /// The values this preset gives `fixture`, if any.
    pub fn values_for(&self, fixture: u32) -> Option<&Values> {
        self.fixtures
            .get(&fixture)
            .or((!self.all.is_empty()).then_some(&self.all))
    }

    /// Applies the preset to the programmer's selection.
    pub fn apply(&self, programmer: &mut Programmer) {
        for id in programmer.selection.clone() {
            if let Some(values) = self.values_for(id) {
                let values = values.only(self.family);
                programmer.values.entry(id).or_default().merge(&values);
            }
        }
    }
}

/// The lowest number not yet used by `ids`, starting at 1.
pub fn next_id(ids: impl Iterator<Item = u32>) -> u32 {
    let mut used: Vec<u32> = ids.collect();
    used.sort_unstable();
    let mut next = 1;
    for id in used {
        if id == next {
            next += 1;
        } else if id > next {
            break;
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Library;

    fn mode(profile: &str, mode: &str) -> Mode {
        Library::with_bundled().mode(profile, mode).unwrap().clone()
    }

    fn render(values: &Values, profile: &str, name: &str) -> Vec<Option<u8>> {
        values.render(&mode(profile, name))
    }

    const ORANGE: Rgb = Rgb { r: 1.0, g: 0.5, b: 0.0 };

    fn warm_cool(attributes: &[Attribute]) -> Mode {
        Mode {
            name: "test".into(),
            channels: attributes
                .iter()
                .map(|attribute| Channel {
                    name: format!("{attribute:?}"),
                    attribute: *attribute,
                    fine_of: None,
                    default: 0,
                    highlight: None,
                    capabilities: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn white_balance_cross_fades_warm_and_cool() {
        let with_dimmer = warm_cool(&[Attribute::Intensity, Attribute::WarmWhite, Attribute::CoolWhite]);
        let balanced = |balance: f32, intensity: Option<f32>| Values {
            intensity,
            other: [(Attribute::ColorTemperature, balance)].into(),
            ..Default::default()
        };
        assert_eq!(
            balanced(0.0, Some(1.0)).render(&with_dimmer),
            vec![Some(255), Some(255), Some(0)]
        );
        assert_eq!(
            balanced(0.5, Some(1.0)).render(&with_dimmer),
            vec![Some(255), Some(255), Some(255)]
        );
        assert_eq!(
            balanced(0.75, Some(1.0)).render(&with_dimmer),
            vec![Some(255), Some(128), Some(255)]
        );

        // Without a dimmer the intensity scales both emitters.
        let no_dimmer = warm_cool(&[Attribute::WarmWhite, Attribute::CoolWhite]);
        assert_eq!(balanced(1.0, Some(0.5)).render(&no_dimmer), vec![Some(0), Some(128)]);
    }

    #[test]
    fn white_balance_channel_is_set_directly() {
        let values = Values {
            other: [(Attribute::ColorTemperature, 1.0)].into(),
            ..Default::default()
        };
        let out = render(&values, "generic/bi-colour-led-panel", "2ch");
        assert_eq!(out, vec![None, Some(255)]);
    }

    #[test]
    fn rgbw_takes_white_from_the_mix() {
        let values = Values {
            intensity: Some(1.0),
            colour: Some(Rgb { r: 1.0, g: 0.6, b: 0.6 }),
            ..Default::default()
        };
        let out = render(&values, "generic/rgbw-par", "6ch DRGBW Strobe");
        assert_eq!(out, vec![Some(255), Some(102), Some(0), Some(0), Some(153), None]);
    }

    #[test]
    fn no_dimmer_scales_the_mix() {
        let values = Values {
            intensity: Some(0.5),
            colour: Some(ORANGE),
            ..Default::default()
        };
        assert_eq!(
            render(&values, "generic/rgb-par", "3ch RGB"),
            vec![Some(128), Some(64), Some(0)]
        );
        // Intensity alone gives white.
        let values = Values {
            intensity: Some(1.0),
            ..Default::default()
        };
        assert_eq!(
            render(&values, "generic/rgb-par", "3ch RGB"),
            vec![Some(255), Some(255), Some(255)]
        );
    }

    #[test]
    fn position_uses_fine_channels() {
        let values = Values {
            pan: Some(0.5),
            tilt: Some(1.0),
            ..Default::default()
        };
        let out = render(&values, "generic/moving-head-spot", "12ch");
        assert_eq!(&out[0..4], &[Some(128), Some(0), Some(255), Some(255)]);
        assert_eq!(out[5], None); // dimmer untouched
    }

    #[test]
    fn colour_wheel_picks_nearest_slot() {
        let values = Values {
            colour: Some(Rgb { r: 0.1, g: 0.1, b: 0.9 }),
            ..Default::default()
        };
        let out = render(&values, "generic/moving-head-spot", "12ch");
        assert_eq!(out[7], Some(103)); // Blue, 96-111
        let values = Values {
            colour: Some(ORANGE),
            ..Default::default()
        };
        assert_eq!(render(&values, "generic/moving-head-spot", "12ch")[7], Some(39));
    }

    #[test]
    fn other_attributes_and_dimmer_only() {
        let mut values = Values {
            intensity: Some(0.25),
            ..Default::default()
        };
        values.other.insert(Attribute::Zoom, 1.0);
        let out = render(&values, "generic/moving-head-wash", "12ch");
        assert_eq!(out[5], Some(64));
        assert_eq!(out[11], Some(255));
        assert_eq!(render(&values, "generic/dimmer", "2ch 16-bit"), vec![Some(64), Some(0)]);
    }

    #[test]
    fn selection_modes() {
        let mut programmer = Programmer::default();
        programmer.select(&[3, 1], SelectMode::Set);
        programmer.select(&[2, 1], SelectMode::Add);
        assert_eq!(programmer.selection, vec![3, 1, 2]);
        programmer.select(&[1, 4], SelectMode::Toggle);
        assert_eq!(programmer.selection, vec![3, 2, 4]);
        programmer.retain(|id| id != 2);
        assert_eq!(programmer.selection, vec![3, 4]);
    }

    #[test]
    fn clear_by_family() {
        let mut programmer = Programmer::default();
        programmer.select(&[1], SelectMode::Set);
        programmer.update(|v| {
            v.intensity = Some(1.0);
            v.colour = Some(ORANGE);
            v.other.insert(Attribute::ColorWheel, 0.2);
            v.other.insert(Attribute::Zoom, 0.2);
        });
        programmer.clear(Some(Family::Colour));
        let values = &programmer.values[&1];
        assert_eq!(values.colour, None);
        assert_eq!(values.other.keys().collect::<Vec<_>>(), vec![&Attribute::Zoom]);
        programmer.clear(Some(Family::Intensity));
        programmer.clear(Some(Family::Beam));
        assert!(programmer.values.is_empty());
    }

    #[test]
    fn shared_preset_applies_to_any_fixture() {
        let mut programmer = Programmer::default();
        programmer.select(&[1, 2], SelectMode::Set);
        programmer.update(|v| {
            v.colour = Some(ORANGE);
            v.intensity = Some(1.0);
        });
        let preset = Preset::record(1, "Orange".into(), Family::Colour, &programmer).unwrap();
        assert_eq!(preset.all.colour, Some(ORANGE));
        assert_eq!(preset.all.intensity, None);
        assert!(preset.fixtures.is_empty());

        let mut other = Programmer::default();
        other.select(&[7], SelectMode::Set);
        preset.apply(&mut other);
        assert_eq!(other.values[&7].colour, Some(ORANGE));
    }

    #[test]
    fn position_preset_is_per_fixture() {
        let mut programmer = Programmer::default();
        programmer.select(&[1], SelectMode::Set);
        programmer.update(|v| v.pan = Some(0.2));
        programmer.select(&[2], SelectMode::Set);
        programmer.update(|v| v.pan = Some(0.8));
        programmer.select(&[1, 2], SelectMode::Set);
        let preset = Preset::record(1, "Stage".into(), Family::Position, &programmer).unwrap();
        assert_eq!(preset.fixtures.len(), 2);

        let mut other = Programmer::default();
        other.select(&[2, 3], SelectMode::Set);
        preset.apply(&mut other);
        assert_eq!(other.values[&2].pan, Some(0.8));
        assert!(!other.values.contains_key(&3));
    }

    #[test]
    fn recording_nothing_fails() {
        let programmer = Programmer::default();
        assert!(matches!(
            Preset::record(1, "x".into(), Family::Colour, &programmer),
            Err(Error::NothingToRecord)
        ));
    }

    #[test]
    fn next_ids() {
        assert_eq!(next_id([].into_iter()), 1);
        assert_eq!(next_id([1, 2, 4].into_iter()), 3);
        assert_eq!(next_id([2, 3].into_iter()), 1);
    }
}
