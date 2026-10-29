//! Import of fixture definitions from the Open Fixture Library
//! (<https://open-fixture-library.org>), using its JSON format.
//!
//! OFL describes far more than HumLX needs, so the import keeps the channel
//! layout of each mode, an attribute per channel and the labelled DMX ranges.
//! Matrix (pixel) fixtures are not supported yet.

use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::fixture::{Attribute, Capability, Channel, Mode, Profile};

/// Converts an OFL fixture JSON document into a profile.
///
/// OFL fixture files do not name their manufacturer (it comes from the
/// folder they live in), so it can be passed in. When it is `None` the
/// `manufacturerKey` field written by OFL's export is used if present.
pub fn import(json: &str, manufacturer: Option<&str>) -> Result<Profile> {
    let root: Value = serde_json::from_str(json).map_err(|e| invalid(format!("not valid JSON: {e}")))?;
    let root = root.as_object().ok_or_else(|| invalid("expected a JSON object"))?;

    let model = str_field(root, "name").ok_or_else(|| invalid("missing fixture \"name\""))?;
    let manufacturer = manufacturer
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_string)
        .or_else(|| str_field(root, "manufacturerKey").map(|key| title_case(&key)))
        .ok_or_else(|| invalid("the manufacturer could not be found in the file, so it must be given"))?;

    if root.contains_key("matrix") || root.contains_key("templateChannels") {
        return Err(Error::Unsupported("matrix (pixel) fixtures".into()));
    }

    let channels = available_channels(root)?;
    let modes = root
        .get("modes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing \"modes\""))?
        .iter()
        .map(|mode| import_mode(mode, &channels))
        .collect::<Result<Vec<_>>>()?;

    let categories = root
        .get("categories")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();

    let profile = Profile {
        manufacturer,
        model,
        categories,
        modes,
    };
    profile.validate()?;
    Ok(profile)
}

fn import_mode(mode: &Value, channels: &HashMap<String, Channel>) -> Result<Mode> {
    let name = mode
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("a mode is missing its \"name\""))?;
    let channels = mode
        .get("channels")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("mode \"{name}\" has no channels")))?
        .iter()
        .map(|entry| match entry {
            Value::String(key) => Ok(channels.get(key).cloned().unwrap_or_else(|| generic(key))),
            Value::Null => Ok(generic("No function")),
            _ => Err(Error::Unsupported("matrix channel inserts in modes".into())),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Mode {
        name: name.to_string(),
        channels,
    })
}

/// Builds every channel the fixture defines, including fine channel aliases,
/// keyed by the name modes use to refer to them.
fn available_channels(root: &Map<String, Value>) -> Result<HashMap<String, Channel>> {
    let wheels = root.get("wheels").and_then(Value::as_object);
    let mut channels = HashMap::new();
    let Some(available) = root.get("availableChannels").and_then(Value::as_object) else {
        return Ok(channels);
    };
    for (key, definition) in available {
        let definition = definition
            .as_object()
            .ok_or_else(|| invalid(format!("channel \"{key}\" is not an object")))?;
        let capabilities = raw_capabilities(definition);
        let attribute = infer_attribute(key, &capabilities, wheels);

        let channel = Channel {
            name: display_name(key, definition),
            attribute,
            fine_of: None,
            default: dmx_value(definition.get("defaultValue")).unwrap_or(0),
            highlight: dmx_value(definition.get("highlightValue")),
            capabilities: capabilities
                .iter()
                .filter_map(|c| capability(c, capabilities.len()))
                .collect(),
        };

        if let Some(aliases) = definition.get("fineChannelAliases").and_then(Value::as_array) {
            for alias in aliases.iter().filter_map(Value::as_str) {
                channels.insert(
                    alias.to_string(),
                    Channel {
                        name: alias.to_string(),
                        attribute,
                        fine_of: Some(channel.name.clone()),
                        default: 0,
                        highlight: None,
                        capabilities: vec![],
                    },
                );
            }
        }
        channels.insert(key.clone(), channel);
    }
    Ok(channels)
}

fn display_name(key: &str, definition: &Map<String, Value>) -> String {
    str_field(definition, "name").unwrap_or_else(|| key.to_string())
}

fn raw_capabilities(definition: &Map<String, Value>) -> Vec<&Map<String, Value>> {
    if let Some(single) = definition.get("capability").and_then(Value::as_object) {
        return vec![single];
    }
    definition
        .get("capabilities")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_object).collect())
        .unwrap_or_default()
}

fn capability(raw: &Map<String, Value>, count: usize) -> Option<Capability> {
    // A single capability spanning the whole channel adds nothing to show.
    if count < 2 {
        return None;
    }
    let range = raw.get("dmxRange")?.as_array()?;
    let from = range.first()?.as_u64()?.min(255) as u8;
    let to = range.get(1)?.as_u64()?.min(255) as u8;
    Some(Capability {
        from,
        to,
        label: capability_label(raw),
    })
}

fn capability_label(raw: &Map<String, Value>) -> String {
    if let Some(comment) = str_field(raw, "comment") {
        return comment;
    }
    let kind = str_field(raw, "type").unwrap_or_else(|| "Generic".into());
    if let Some(slot) = raw.get("slotNumber").and_then(Value::as_f64) {
        return format!("Slot {slot}");
    }
    if let Some(color) = str_field(raw, "color") {
        return color;
    }
    if let Some(effect) = str_field(raw, "shutterEffect") {
        return effect;
    }
    split_camel_case(&kind)
}

/// Picks the attribute from the first meaningful capability, falling back to
/// the channel name for wheels and anything ambiguous.
fn infer_attribute(key: &str, capabilities: &[&Map<String, Value>], wheels: Option<&Map<String, Value>>) -> Attribute {
    let name = key.to_lowercase();
    for raw in capabilities {
        let Some(kind) = raw.get("type").and_then(Value::as_str) else {
            continue;
        };
        let attribute = match kind {
            "Intensity" => Some(Attribute::Intensity),
            "ColorIntensity" => raw.get("color").and_then(Value::as_str).map(colour_attribute),
            "ColorPreset" => Some(Attribute::ColorWheel),
            "ColorTemperature" => Some(Attribute::ColorTemperature),
            "Pan" | "PanContinuous" => Some(Attribute::Pan),
            "Tilt" | "TiltContinuous" => Some(Attribute::Tilt),
            "PanTiltSpeed" => Some(Attribute::PanTiltSpeed),
            "ShutterStrobe" | "StrobeSpeed" | "StrobeDuration" => Some(Attribute::Shutter),
            "WheelSlot" | "WheelShake" | "WheelSlotRotation" | "WheelRotation" => {
                let wheel = raw
                    .get("wheel")
                    .and_then(Value::as_str)
                    .map(str::to_lowercase)
                    .unwrap_or_else(|| name.clone());
                Some(wheel_attribute(&wheel, kind, wheels))
            }
            "Prism" | "PrismRotation" => Some(Attribute::Prism),
            "Zoom" | "BeamAngle" => Some(Attribute::Zoom),
            "Focus" => Some(Attribute::Focus),
            "Iris" | "IrisEffect" => Some(Attribute::Iris),
            "Frost" | "FrostEffect" => Some(Attribute::Frost),
            "Effect"
            | "EffectParameter"
            | "EffectDuration"
            | "SoundSensitivity"
            | "BladeInsertion"
            | "BladeRotation"
            | "BladeSystemRotation" => Some(Attribute::Effect),
            "EffectSpeed" | "Speed" => Some(Attribute::Speed),
            "Maintenance" => Some(Attribute::Control),
            _ => None,
        };
        if let Some(attribute) = attribute {
            return attribute;
        }
    }
    Attribute::Generic
}

fn wheel_attribute(wheel: &str, kind: &str, wheels: Option<&Map<String, Value>>) -> Attribute {
    let rotating = kind.contains("Rotation");
    if wheel.contains("colo") {
        return Attribute::ColorWheel;
    }
    if wheel.contains("gobo") {
        return if rotating {
            Attribute::GoboRotation
        } else {
            Attribute::Gobo
        };
    }
    if wheel.contains("prism") {
        return Attribute::Prism;
    }
    // Fall back to the slots defined on the wheel itself.
    let slot_types: Vec<&str> = wheels
        .and_then(|w| w.iter().find(|(name, _)| name.to_lowercase() == wheel))
        .and_then(|(_, w)| w.get("slots"))
        .and_then(Value::as_array)
        .map(|slots| {
            slots
                .iter()
                .filter_map(|s| s.get("type").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    if slot_types.contains(&"Color") {
        Attribute::ColorWheel
    } else if slot_types.contains(&"Gobo") {
        if rotating {
            Attribute::GoboRotation
        } else {
            Attribute::Gobo
        }
    } else {
        Attribute::Effect
    }
}

fn colour_attribute(color: &str) -> Attribute {
    match color {
        "Red" => Attribute::Red,
        "Green" => Attribute::Green,
        "Blue" => Attribute::Blue,
        "White" => Attribute::White,
        "Warm White" => Attribute::WarmWhite,
        "Cold White" => Attribute::CoolWhite,
        "Amber" => Attribute::Amber,
        "Lime" => Attribute::Lime,
        "UV" => Attribute::Uv,
        "Cyan" => Attribute::Cyan,
        "Magenta" => Attribute::Magenta,
        "Yellow" => Attribute::Yellow,
        _ => Attribute::Generic,
    }
}

/// Reads an OFL DMX value, which is either a number or a percentage string.
fn dmx_value(value: Option<&Value>) -> Option<u8> {
    match value? {
        Value::Number(n) => n.as_u64().map(|v| v.min(255) as u8),
        Value::String(s) => {
            let percent: f64 = s.strip_suffix('%')?.trim().parse().ok()?;
            Some((percent.clamp(0.0, 100.0) / 100.0 * 255.0).round() as u8)
        }
        _ => None,
    }
}

fn generic(name: &str) -> Channel {
    Channel {
        name: name.to_string(),
        attribute: Attribute::Generic,
        fine_of: None,
        default: 0,
        highlight: None,
        capabilities: vec![],
    }
}

fn str_field(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(Value::as_str).map(str::to_string)
}

fn title_case(key: &str) -> String {
    key.split(['-', '_', ' '])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn split_camel_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    for (i, c) in text.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidProfile(message.into())
}
