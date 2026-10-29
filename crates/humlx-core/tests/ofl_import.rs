use humlx_core::fixture::Attribute;
use humlx_core::ofl;

const SPOT: &str = include_str!("data/ofl-spot.json");

#[test]
fn imports_modes_and_attributes() {
    let profile = ofl::import(SPOT, Some("Test Co")).unwrap();
    assert_eq!(profile.manufacturer, "Test Co");
    assert_eq!(profile.model, "Test Spot 150");
    assert_eq!(profile.id(), "test-co/test-spot-150");
    assert_eq!(profile.categories, vec!["Moving Head", "Color Changer"]);

    let basic = profile.mode("Basic").unwrap();
    let attributes: Vec<_> = basic.channels.iter().map(|c| c.attribute).collect();
    assert_eq!(
        attributes,
        vec![
            Attribute::Pan,
            Attribute::Tilt,
            Attribute::Intensity,
            Attribute::Shutter,
            Attribute::ColorWheel,
            Attribute::Gobo,
            Attribute::Generic,
            Attribute::Effect,
        ]
    );
    assert_eq!(basic.channels[6].name, "No function");
}

#[test]
fn imports_fine_channels_and_values() {
    let profile = ofl::import(SPOT, Some("Test Co")).unwrap();
    let extended = profile.mode("Extended").unwrap();
    assert_eq!(extended.channels.len(), 11);

    let pan = &extended.channels[0];
    assert_eq!(pan.default, 128); // "50%"
    let pan_fine = &extended.channels[1];
    assert_eq!(pan_fine.attribute, Attribute::Pan);
    assert_eq!(pan_fine.fine_of.as_deref(), Some("Pan"));

    let shutter = &extended.channels[5];
    assert_eq!(shutter.highlight_value(), 255);
    let labels: Vec<_> = shutter.capabilities.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Closed", "Strobe", "Open"]);
    assert_eq!((shutter.capabilities[1].from, shutter.capabilities[1].to), (32, 223));

    let wheel = &extended.channels[6];
    assert_eq!(wheel.capabilities[2].label, "Rainbow");

    assert_eq!(extended.channels[8].attribute, Attribute::Red);
    // Channels a mode names but the file does not define are kept as generic.
    assert_eq!(extended.channels[10].name, "Mystery");
    assert_eq!(extended.channels[10].attribute, Attribute::Generic);
}

#[test]
fn needs_a_manufacturer() {
    assert!(ofl::import(SPOT, None).is_err());
    assert!(ofl::import(SPOT, Some("  ")).is_err());
    let with_key = SPOT.replacen('{', r#"{ "manufacturerKey": "acme-lighting","#, 1);
    assert_eq!(ofl::import(&with_key, None).unwrap().manufacturer, "Acme Lighting");
}

#[test]
fn rejects_matrix_fixtures_and_bad_json() {
    let matrix = SPOT.replacen('{', r#"{ "matrix": { "pixelCount": [8, 1, 1] },"#, 1);
    assert!(ofl::import(&matrix, Some("Test Co")).is_err());
    assert!(ofl::import("not json", Some("Test Co")).is_err());
    assert!(ofl::import("{}", Some("Test Co")).is_err());
}
