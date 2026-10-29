//! Import of QLC+ fixture definitions (`.qxf` XML files), from QLC+'s own
//! library or made by hand in its fixture editor.
//!
//! Like the OFL import, this keeps each mode's channel layout, an attribute
//! per channel, fine channels and the labelled DMX ranges. Attributes come
//! from the channel's preset (QLC+ 4.12 and later) or else its group, with the
//! channel name to settle anything ambiguous.

use std::collections::HashMap;

use roxmltree::{Document, Node, ParsingOptions};

use crate::error::{Error, Result};
use crate::fixture::{Attribute, Capability, Channel, Mode, Profile};

/// A channel as QLC+ defines it, before modes place it.
struct Definition {
    channel: Channel,
    fine: bool,
}

/// Converts a QLC+ fixture definition into a profile. The manufacturer is read
/// from the file unless one is given.
pub fn import(xml: &str, manufacturer: Option<&str>) -> Result<Profile> {
    // QLC+ files start with `<!DOCTYPE FixtureDefinition>`, which has no entities.
    let options = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    let doc = Document::parse_with_options(xml, options)
        .map_err(|e| invalid(format!("not a valid QLC+ fixture file: {e}")))?;
    let root = doc.root_element();
    if root.tag_name().name() != "FixtureDefinition" {
        return Err(invalid("not a QLC+ fixture definition"));
    }

    let model = child_text(root, "Model").ok_or_else(|| invalid("missing <Model>"))?;
    let manufacturer = manufacturer
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_string)
        .or_else(|| child_text(root, "Manufacturer"))
        .ok_or_else(|| invalid("missing <Manufacturer>"))?;

    let definitions: HashMap<String, Definition> = children(root, "Channel")
        .filter_map(|node| {
            let name = node.attribute("Name")?.trim().to_string();
            Some((name.clone(), definition(node, name)))
        })
        .collect();

    let modes = children(root, "Mode")
        .map(|node| import_mode(node, &definitions))
        .collect::<Result<Vec<_>>>()?;
    if modes.is_empty() {
        return Err(invalid("the file has no modes"));
    }

    let profile = Profile {
        manufacturer,
        model,
        categories: child_text(root, "Type").into_iter().collect(),
        modes,
    };
    profile.validate()?;
    Ok(profile)
}

fn import_mode(node: Node, definitions: &HashMap<String, Definition>) -> Result<Mode> {
    let name = node
        .attribute("Name")
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .ok_or_else(|| invalid("a mode is missing its name"))?;

    let mut placed: Vec<(usize, String)> = children(node, "Channel")
        .filter_map(|c| {
            let number = c.attribute("Number")?.parse().ok()?;
            Some((number, c.text().unwrap_or("").trim().to_string()))
        })
        .collect();
    placed.sort_by_key(|(number, _)| *number);
    if placed.is_empty() {
        return Err(invalid(format!("mode \"{name}\" has no channels")));
    }

    let mut channels: Vec<Channel> = Vec::with_capacity(placed.len());
    for (_, channel_name) in &placed {
        let Some(def) = definitions.get(channel_name) else {
            channels.push(generic(channel_name));
            continue;
        };
        let mut channel = def.channel.clone();
        if def.fine {
            // Pair with the nearest earlier coarse channel of the same
            // attribute, or any later one (some fixtures put fine channels first).
            let coarse = |c: &&Channel| c.fine_of.is_none() && c.attribute == channel.attribute;
            let earlier = channels.iter().rev().find(coarse).map(|c| c.name.clone());
            let later = || {
                placed
                    .iter()
                    .filter_map(|(_, n)| definitions.get(n))
                    .filter(|d| !d.fine)
                    .map(|d| &d.channel)
                    .find(|c| c.attribute == channel.attribute)
                    .map(|c| c.name.clone())
            };
            channel.fine_of = earlier.or_else(later);
            if channel.fine_of.is_some() {
                channel.capabilities.clear();
            }
        }
        channels.push(channel);
    }

    Ok(Mode {
        name: name.to_string(),
        channels,
    })
}

fn definition(node: Node, name: String) -> Definition {
    let preset = node.attribute("Preset").unwrap_or("");
    let group = children(node, "Group").next();
    let group_name = group.and_then(|g| g.text()).unwrap_or("").trim();
    let byte_fine = group.and_then(|g| g.attribute("Byte")) == Some("1");
    let colour = child_text(node, "Colour");

    let attribute = from_preset(preset).unwrap_or_else(|| from_group(group_name, colour.as_deref(), &name));
    let capabilities: Vec<Capability> = children(node, "Capability")
        .filter_map(|c| {
            let from = c.attribute("Min")?.parse::<u8>().ok()?;
            let to = c.attribute("Max")?.parse::<u8>().ok()?;
            let label = c.text().unwrap_or("").trim().to_string();
            Some(Capability { from, to, label })
        })
        .filter(|c| !c.label.is_empty())
        .collect();

    Definition {
        fine: byte_fine || preset.ends_with("Fine"),
        channel: Channel {
            name,
            attribute,
            fine_of: None,
            default: node.attribute("Default").and_then(|d| d.parse().ok()).unwrap_or(0),
            highlight: None,
            capabilities,
        },
    }
}

/// Channel presets from QLC+ 4.12 and later.
fn from_preset(preset: &str) -> Option<Attribute> {
    use Attribute::*;
    let base = preset.strip_suffix("Fine").unwrap_or(preset);
    Some(match base {
        "" => return None,
        "IntensityMasterDimmer" | "IntensityDimmer" => Intensity,
        "IntensityRed" => Red,
        "IntensityGreen" => Green,
        "IntensityBlue" => Blue,
        "IntensityWhite" => White,
        "IntensityAmber" => Amber,
        "IntensityUV" => Uv,
        "IntensityLime" => Lime,
        "IntensityCyan" => Cyan,
        "IntensityMagenta" => Magenta,
        "IntensityYellow" => Yellow,
        "PositionPan" | "PositionXAxis" => Pan,
        "PositionTilt" | "PositionYAxis" => Tilt,
        p if p.starts_with("SpeedPan") || p.starts_with("SpeedTilt") => PanTiltSpeed,
        "ColorMacro" | "ColorWheel" | "ColorRGBMixer" => ColorWheel,
        "ColorCTOMixer" | "ColorCTCMixer" | "ColorCTBMixer" => ColorTemperature,
        "GoboWheel" => Gobo,
        "GoboIndex" => GoboRotation,
        p if p.starts_with("ShutterStrobe") => Shutter,
        p if p.starts_with("ShutterIris") => Iris,
        p if p.starts_with("BeamFocus") => Focus,
        p if p.starts_with("BeamZoom") => Zoom,
        p if p.starts_with("Prism") => Prism,
        "NoFunction" => Generic,
        _ => return None,
    })
}

/// Channel groups, used by older files and by channels without a preset.
fn from_group(group: &str, colour: Option<&str>, name: &str) -> Attribute {
    use Attribute::*;
    let name = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    match group {
        "Intensity" => match colour.map(str::to_lowercase).as_deref() {
            Some("red") => Red,
            Some("green") => Green,
            Some("blue") => Blue,
            Some("white") => White,
            Some("amber") => Amber,
            Some("uv") => Uv,
            Some("lime") => Lime,
            Some("cyan") => Cyan,
            Some("magenta") => Magenta,
            Some("yellow") => Yellow,
            _ if has(&["red"]) => Red,
            _ if has(&["green"]) => Green,
            _ if has(&["blue"]) => Blue,
            _ if has(&["white"]) => White,
            _ if has(&["amber"]) => Amber,
            _ if has(&["uv", "ultraviolet"]) => Uv,
            _ => Intensity,
        },
        "Pan" => Pan,
        "Tilt" => Tilt,
        "Colour" if has(&["cto", "ctb", "temperature", "temp"]) => ColorTemperature,
        "Colour" => ColorWheel,
        "Gobo" if has(&["rot", "spin", "index"]) => GoboRotation,
        "Gobo" => Gobo,
        "Prism" => Prism,
        "Shutter" if has(&["iris"]) => Iris,
        "Shutter" => Shutter,
        "Beam" | "Effect" | "Nothing" | "" if has(&["zoom"]) => Zoom,
        "Beam" | "Effect" | "Nothing" | "" if has(&["focus"]) => Focus,
        "Beam" | "Effect" | "Nothing" | "" if has(&["iris"]) => Iris,
        "Beam" | "Effect" | "Nothing" | "" if has(&["frost", "diffus"]) => Frost,
        "Beam" => Effect,
        "Speed" if has(&["pan", "tilt", "p/t", "movement"]) => PanTiltSpeed,
        "Speed" => Speed,
        "Effect" => Effect,
        "Maintenance" => Control,
        _ if has(&["dimmer", "intensity"]) => Intensity,
        _ => Generic,
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

fn children<'a, 'i>(node: Node<'a, 'i>, tag: &'static str) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == tag)
}

fn child_text(node: Node, tag: &'static str) -> Option<String> {
    children(node, tag)
        .next()
        .and_then(|c| c.text())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidProfile(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPOT: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE FixtureDefinition>
<FixtureDefinition xmlns="http://www.qlcplus.org/FixtureDefinition">
 <Creator><Name>Q Light Controller Plus</Name><Version>4.12.7</Version><Author>Test</Author></Creator>
 <Manufacturer>Event Lighting</Manufacturer>
 <Model>Test Spot</Model>
 <Type>Moving Head</Type>
 <Channel Name="Pan" Preset="PositionPan"/>
 <Channel Name="Pan fine" Preset="PositionPanFine"/>
 <Channel Name="Tilt"><Group Byte="0">Tilt</Group></Channel>
 <Channel Name="Tilt fine"><Group Byte="1">Tilt</Group></Channel>
 <Channel Name="Dimmer" Default="0"><Group Byte="0">Intensity</Group>
  <Capability Min="0" Max="255">Intensity</Capability></Channel>
 <Channel Name="Red"><Group Byte="0">Intensity</Group><Colour>Red</Colour></Channel>
 <Channel Name="Green" Preset="IntensityGreen"/>
 <Channel Name="Blue" Preset="IntensityBlue"/>
 <Channel Name="White" Preset="IntensityWhite"/>
 <Channel Name="Colour wheel"><Group Byte="0">Colour</Group>
  <Capability Min="0" Max="15" Preset="ColorMacro" Res1="#ffffff">White</Capability>
  <Capability Min="16" Max="31" Preset="ColorMacro" Res1="#ff0000">Red</Capability>
 </Channel>
 <Channel Name="Strobe"><Group Byte="0">Shutter</Group>
  <Capability Min="0" Max="9">Open</Capability>
  <Capability Min="10" Max="255">Strobe slow to fast</Capability>
 </Channel>
 <Channel Name="Pan/Tilt speed"><Group Byte="0">Speed</Group></Channel>
 <Channel Name="Reset" Default="0"><Group Byte="0">Maintenance</Group></Channel>
 <Mode Name="9 Channel">
  <Channel Number="0">Pan</Channel>
  <Channel Number="1">Tilt</Channel>
  <Channel Number="2">Dimmer</Channel>
  <Channel Number="3">Red</Channel>
  <Channel Number="4">Green</Channel>
  <Channel Number="5">Blue</Channel>
  <Channel Number="6">White</Channel>
  <Channel Number="7">Strobe</Channel>
  <Channel Number="8">Pan/Tilt speed</Channel>
 </Mode>
 <Mode Name="14 Channel">
  <Channel Number="0">Pan</Channel>
  <Channel Number="1">Tilt</Channel>
  <Channel Number="2">Pan fine</Channel>
  <Channel Number="3">Tilt fine</Channel>
  <Channel Number="4">Dimmer</Channel>
  <Channel Number="5">Red</Channel>
  <Channel Number="6">Green</Channel>
  <Channel Number="7">Blue</Channel>
  <Channel Number="8">White</Channel>
  <Channel Number="9">Colour wheel</Channel>
  <Channel Number="10">Strobe</Channel>
  <Channel Number="11">Pan/Tilt speed</Channel>
  <Channel Number="12">Reset</Channel>
  <Channel Number="13">Unknown</Channel>
 </Mode>
</FixtureDefinition>"##;

    #[test]
    fn imports_modes_attributes_and_fine_channels() {
        let profile = import(SPOT, None).unwrap();
        assert_eq!(profile.manufacturer, "Event Lighting");
        assert_eq!(profile.model, "Test Spot");
        assert_eq!(profile.categories, vec!["Moving Head".to_string()]);
        assert_eq!(profile.modes.len(), 2);

        let small = &profile.modes[0];
        let attrs: Vec<Attribute> = small.channels.iter().map(|c| c.attribute).collect();
        use Attribute::*;
        assert_eq!(
            attrs,
            vec![Pan, Tilt, Intensity, Red, Green, Blue, White, Shutter, PanTiltSpeed]
        );

        let full = &profile.modes[1];
        assert_eq!(full.channels.len(), 14);
        assert_eq!(full.channels[2].fine_of.as_deref(), Some("Pan"));
        assert_eq!(full.channels[3].fine_of.as_deref(), Some("Tilt"));
        assert_eq!(full.channels[9].attribute, ColorWheel);
        assert_eq!(full.channels[9].capabilities[1].label, "Red");
        assert_eq!(full.channels[10].capabilities[0].to, 9);
        assert_eq!(full.channels[12].attribute, Control);
        assert_eq!(full.channels[13].attribute, Generic);
    }

    #[test]
    fn manufacturer_can_be_overridden() {
        assert_eq!(import(SPOT, Some("Event")).unwrap().manufacturer, "Event");
    }

    #[test]
    fn rejects_other_xml() {
        assert!(import("<Workspace/>", None).is_err());
        assert!(import("not xml", None).is_err());
    }
}
