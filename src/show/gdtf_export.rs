//! GDTF (`.gdtf`) fixture-type export — companion to the ASCII export.
//!
//! USITT ASCII has no way to describe a multi-parameter fixture personality
//! from scratch (see `ascii_export.rs`'s `KNOWN_PERSONALITIES` limitation) —
//! consoles resolve `$Personality` blocks against their own fixture library,
//! never build one from the file. GDTF is a real, self-contained fixture-type
//! definition (a zip archive holding `description.xml`) that GDTF-aware
//! consoles (e.g. ETC Eos via **File > Import > Fixture**) parse directly, so
//! it can describe genuinely custom channel layouts. The MVR exporter embeds
//! these same GDTF files, and the ASCII `MvrMatch` export references the same
//! fixture identity (`gdtf_guid`/`profile_identity`) so EOS resolves its
//! `$Personality` blocks to the already-imported types.
//!
//! This module builds one minimal, spec-compliant GDTF per distinct
//! `FixtureProfile`: a single `Geometry`, one `DMXMode` with one `DMXChannel`
//! per mappable parameter (Pan/Tilt merge with their Fine channel into a
//! single 16-bit `Offset="coarse,fine"` channel — GDTF supports this natively,
//! unlike EOS ASCII). Parameters with no GDTF equivalent (`Custom`) are
//! omitted from the DMX mode.

use crate::fixtures::profiles::{FixtureParameter, FixtureProfile};
use std::fmt::Write as _;
use std::io::Write as _;

/// A GDTF-standard Fixture Type Attribute (see GDTF spec Annex A/B) used by
/// one of our parameters.
struct GdtfAttr {
    name: &'static str,
    pretty: &'static str,
    feature_group: &'static str,
    feature: &'static str,
    activation_group: Option<&'static str>,
    physical_unit: &'static str,
}

/// Map an EasyCue3 fixture parameter to its GDTF Fixture Type Attribute.
/// Returns `None` for parameters folded into another channel (Pan/TiltFine)
/// or with no GDTF equivalent (`Custom`).
fn gdtf_attr(p: &FixtureParameter) -> Option<GdtfAttr> {
    use FixtureParameter as F;
    Some(match p {
        F::Intensity => GdtfAttr {
            name: "Dimmer",
            pretty: "Dim",
            feature_group: "Dimmer",
            feature: "Dimmer",
            activation_group: None,
            physical_unit: "None",
        },
        F::Pan => GdtfAttr {
            name: "Pan",
            pretty: "P",
            feature_group: "Position",
            feature: "PanTilt",
            activation_group: Some("PanTilt"),
            physical_unit: "Angle",
        },
        F::Tilt => GdtfAttr {
            name: "Tilt",
            pretty: "T",
            feature_group: "Position",
            feature: "PanTilt",
            activation_group: Some("PanTilt"),
            physical_unit: "Angle",
        },
        F::Red => GdtfAttr {
            name: "ColorAdd_R",
            pretty: "R",
            feature_group: "Color",
            feature: "RGB",
            activation_group: Some("ColorRGB"),
            physical_unit: "ColorComponent",
        },
        F::Green => GdtfAttr {
            name: "ColorAdd_G",
            pretty: "G",
            feature_group: "Color",
            feature: "RGB",
            activation_group: Some("ColorRGB"),
            physical_unit: "ColorComponent",
        },
        F::Blue => GdtfAttr {
            name: "ColorAdd_B",
            pretty: "B",
            feature_group: "Color",
            feature: "RGB",
            activation_group: Some("ColorRGB"),
            physical_unit: "ColorComponent",
        },
        // Standard GDTF attribute for amber is "ColorAdd_RY" (pretty "Amber"),
        // not "ColorAdd_A" — see spec Annex B.
        F::Amber => GdtfAttr {
            name: "ColorAdd_RY",
            pretty: "Amber",
            feature_group: "Color",
            feature: "RGB",
            activation_group: Some("ColorRGB"),
            physical_unit: "ColorComponent",
        },
        F::White => GdtfAttr {
            name: "ColorAdd_W",
            pretty: "White",
            feature_group: "Color",
            feature: "RGB",
            activation_group: Some("ColorRGB"),
            physical_unit: "ColorComponent",
        },
        F::Uv => GdtfAttr {
            name: "ColorAdd_UV",
            pretty: "UV",
            feature_group: "Color",
            feature: "RGB",
            activation_group: Some("ColorRGB"),
            physical_unit: "ColorComponent",
        },
        F::Strobe => GdtfAttr {
            name: "Shutter1Strobe",
            pretty: "Strobe",
            feature_group: "Beam",
            feature: "Beam",
            activation_group: None,
            physical_unit: "Frequency",
        },
        F::Iris => GdtfAttr {
            name: "Iris",
            pretty: "Iris",
            feature_group: "Beam",
            feature: "Beam",
            activation_group: None,
            physical_unit: "None",
        },
        F::Zoom => GdtfAttr {
            name: "Zoom",
            pretty: "Zoom",
            feature_group: "Focus",
            feature: "Focus",
            activation_group: None,
            physical_unit: "Angle",
        },
        F::Focus => GdtfAttr {
            name: "Focus1",
            pretty: "Focus",
            feature_group: "Focus",
            feature: "Focus",
            activation_group: None,
            physical_unit: "None",
        },
        F::Gobo => GdtfAttr {
            name: "Gobo1",
            pretty: "Gobo",
            feature_group: "Gobo",
            feature: "Gobo",
            activation_group: Some("Gobo1"),
            physical_unit: "None",
        },
        F::Prism => GdtfAttr {
            name: "Prism1",
            pretty: "Prism",
            feature_group: "Beam",
            feature: "Beam",
            activation_group: Some("Prism"),
            physical_unit: "None",
        },
        F::Frost => GdtfAttr {
            name: "Frost1",
            pretty: "Frost",
            feature_group: "Beam",
            feature: "Beam",
            activation_group: None,
            physical_unit: "None",
        },
        F::PanFine | F::TiltFine | F::Custom(_) => return None,
    })
}

/// One DMX channel of the exported GDTF `DMXMode`: an attribute and its
/// 1-based DMX offsets (coarse first, then fine for 16-bit channels).
struct GdtfChannel {
    attr: GdtfAttr,
    offsets: Vec<u16>,
}

/// Build the DMX-mode channel list for a profile, merging Pan/PanFine and
/// Tilt/TiltFine into a single 16-bit channel (`Offset="coarse,fine"`).
fn build_channels(profile: &FixtureProfile) -> Vec<GdtfChannel> {
    let mut out = Vec::new();
    for m in &profile.parameters {
        if matches!(
            m.parameter,
            FixtureParameter::PanFine | FixtureParameter::TiltFine
        ) {
            continue;
        }
        let Some(attr) = gdtf_attr(&m.parameter) else {
            continue;
        };
        let fine = match &m.parameter {
            FixtureParameter::Pan => profile
                .parameters
                .iter()
                .find(|p| p.parameter == FixtureParameter::PanFine),
            FixtureParameter::Tilt => profile
                .parameters
                .iter()
                .find(|p| p.parameter == FixtureParameter::TiltFine),
            _ => None,
        };
        let mut offsets = vec![m.channel_offset + 1];
        if let Some(f) = fine {
            offsets.push(f.channel_offset + 1);
        }
        out.push(GdtfChannel { attr, offsets });
    }
    out
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fnv1a64(seed: u64, data: &[u8]) -> u64 {
    let mut hash = seed ^ 0xcbf29ce484222325;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Deterministic GUID-formatted identifier derived from a profile ID. Only
/// needs to be stable and unique within our own exported files — GDTF import
/// (unlike EOS ASCII `$$Dcid`) does not need to match a console's own
/// pre-existing library.
///
/// Exposed crate-wide so the MVR and ASCII exporters reference the exact same
/// fixture identity (`FixtureTypeID` in the GDTF, `$$Dcid` in ASCII).
pub(crate) fn gdtf_guid(seed: &str) -> String {
    let a = fnv1a64(0x9E37_79B9_7F4A_7C15, seed.as_bytes());
    let b = fnv1a64(0xC2B2_AE3D_27D4_EB4F, seed.as_bytes());
    format!(
        "{:08X}-{:04X}-{:04X}-{:04X}-{:012X}",
        (a >> 32) as u32,
        ((a >> 16) & 0xFFFF) as u16,
        a as u16,
        (b >> 48) as u16,
        b & 0xFFFF_FFFF_FFFF,
    )
}

/// Canonical `(manufacturer, model, DCID)` identity for a profile, shared by
/// the GDTF, MVR and ASCII exporters.  The ASCII `$$Dcid`/`$$Manuf`/`$$Model`
/// must match what the GDTF/MVR defines so EOS resolves the personality to the
/// already-imported fixture type.
pub(crate) fn profile_identity(profile: &FixtureProfile) -> (String, String, String) {
    let manuf = profile
        .manufacturer
        .as_deref()
        .unwrap_or("EasyCue3")
        .to_string();
    let model = profile.name.clone();
    let dcid = gdtf_guid(&profile.id);
    (manuf, model, dcid)
}

/// Explicit physical ranges for attributes where the 0–1 default would be
/// misleading (Pan/Tilt are degrees, not a 0–1 ratio).
fn physical_range(attr: &GdtfAttr) -> String {
    match attr.name {
        "Pan" => " PhysicalFrom=\"-270\" PhysicalTo=\"270\"".to_string(),
        "Tilt" => " PhysicalFrom=\"-135\" PhysicalTo=\"135\"".to_string(),
        _ => String::new(),
    }
}

/// Build the `description.xml` contents for a fixture profile.
fn build_description_xml(profile: &FixtureProfile) -> String {
    let channels = build_channels(profile);

    let mut feature_groups: Vec<(&str, Vec<&str>)> = Vec::new();
    let mut activation_groups: Vec<&str> = Vec::new();
    for ch in &channels {
        match feature_groups
            .iter_mut()
            .find(|(name, _)| *name == ch.attr.feature_group)
        {
            Some((_, features)) => {
                if !features.contains(&ch.attr.feature) {
                    features.push(ch.attr.feature);
                }
            }
            None => feature_groups.push((ch.attr.feature_group, vec![ch.attr.feature])),
        }
        if let Some(ag) = ch.attr.activation_group {
            if !activation_groups.contains(&ag) {
                activation_groups.push(ag);
            }
        }
    }

    let guid = gdtf_guid(&profile.id);
    let name = escape_xml(&profile.name);
    let manuf = escape_xml(profile.manufacturer.as_deref().unwrap_or("EasyCue3"));

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    writeln!(xml, "<GDTF DataVersion=\"1.2\">").unwrap();
    writeln!(
        xml,
        "  <FixtureType Name=\"{name}\" ShortName=\"{name}\" LongName=\"{name}\" Manufacturer=\"{manuf}\" Description=\"Exported from EasyCue3\" FixtureTypeID=\"{guid}\">"
    )
    .unwrap();

    xml.push_str("    <AttributeDefinitions>\n");
    xml.push_str("      <ActivationGroups>\n");
    for ag in &activation_groups {
        writeln!(xml, "        <ActivationGroup Name=\"{ag}\"/>").unwrap();
    }
    xml.push_str("      </ActivationGroups>\n");
    xml.push_str("      <FeatureGroups>\n");
    for (group, features) in &feature_groups {
        writeln!(
            xml,
            "        <FeatureGroup Name=\"{group}\" Pretty=\"{group}\">"
        )
        .unwrap();
        for feature in features {
            writeln!(xml, "          <Feature Name=\"{feature}\"/>").unwrap();
        }
        xml.push_str("        </FeatureGroup>\n");
    }
    xml.push_str("      </FeatureGroups>\n");
    xml.push_str("      <Attributes>\n");
    for ch in &channels {
        let a = &ch.attr;
        write!(
            xml,
            "        <Attribute Name=\"{}\" Pretty=\"{}\" Feature=\"{}.{}\"",
            a.name, a.pretty, a.feature_group, a.feature
        )
        .unwrap();
        if let Some(ag) = a.activation_group {
            write!(xml, " ActivationGroup=\"{ag}\"").unwrap();
        }
        if a.physical_unit != "None" {
            write!(xml, " PhysicalUnit=\"{}\"", a.physical_unit).unwrap();
        }
        xml.push_str("/>\n");
    }
    xml.push_str("      </Attributes>\n");
    xml.push_str("    </AttributeDefinitions>\n");

    xml.push_str("    <Geometries>\n");
    xml.push_str("      <Geometry Name=\"Base\"/>\n");
    xml.push_str("    </Geometries>\n");

    xml.push_str("    <DMXModes>\n");
    xml.push_str("      <DMXMode Name=\"Default\" Geometry=\"Base\">\n");
    xml.push_str("        <DMXChannels>\n");
    for ch in &channels {
        let offsets: Vec<String> = ch.offsets.iter().map(|o| o.to_string()).collect();
        writeln!(
            xml,
            "          <DMXChannel DMXBreak=\"1\" Offset=\"{}\" Geometry=\"Base\">",
            offsets.join(",")
        )
        .unwrap();
        writeln!(
            xml,
            "            <LogicalChannel Attribute=\"{}\">",
            ch.attr.name
        )
        .unwrap();
        let physical = physical_range(&ch.attr);
        writeln!(
            xml,
            "              <ChannelFunction Name=\"{0}\" Attribute=\"{0}\" DMXFrom=\"0/1\"{1}/>",
            ch.attr.name, physical
        )
        .unwrap();
        xml.push_str("            </LogicalChannel>\n");
        xml.push_str("          </DMXChannel>\n");
    }
    xml.push_str("        </DMXChannels>\n");
    xml.push_str("      </DMXMode>\n");
    xml.push_str("    </DMXModes>\n");
    xml.push_str("  </FixtureType>\n");
    xml.push_str("</GDTF>\n");
    xml
}

/// Build a `.gdtf` archive (a zip containing `description.xml`) for a
/// fixture profile.
pub fn export_gdtf(profile: &FixtureProfile) -> Vec<u8> {
    let xml = build_description_xml(profile);
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("description.xml", options)
            .expect("failed to start description.xml in GDTF zip");
        zip.write_all(xml.as_bytes())
            .expect("failed to write description.xml contents");
        zip.finish().expect("failed to finalize GDTF zip");
    }
    buf
}

/// Suggested file name for a profile's `.gdtf`, following the GDTF naming
/// convention `<Manufacturer>@<FixtureTypeName>.gdtf`.
pub fn gdtf_filename(profile: &FixtureProfile) -> String {
    let sanitize = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    let manuf = sanitize(profile.manufacturer.as_deref().unwrap_or("EasyCue3"));
    let model = sanitize(&profile.name);
    format!("{manuf}@{model}.gdtf")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::profiles::ParameterMapping;
    use std::io::Read;

    fn param(parameter: FixtureParameter, offset: u16) -> ParameterMapping {
        ParameterMapping {
            parameter,
            channel_offset: offset,
            default_value: None,
        }
    }

    fn read_description(bytes: &[u8]) -> String {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut file = archive.by_name("description.xml").unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        contents
    }

    #[test]
    fn rgb_profile_produces_valid_zip_with_color_attributes() {
        let profile = FixtureProfile {
            id: "test_rgb".to_string(),
            name: "Test RGB".to_string(),
            manufacturer: None,
            channel_count: 3,
            parameters: vec![
                param(FixtureParameter::Red, 0),
                param(FixtureParameter::Green, 1),
                param(FixtureParameter::Blue, 2),
            ],
            notes: None,
        };
        let bytes = export_gdtf(&profile);
        let xml = read_description(&bytes);
        assert!(xml.contains("ColorAdd_R"));
        assert!(xml.contains("ColorAdd_G"));
        assert!(xml.contains("ColorAdd_B"));
        assert!(xml.contains("Offset=\"1\""));
        assert!(xml.contains("Offset=\"3\""));
    }

    #[test]
    fn moving_head_merges_pan_tilt_fine_into_16bit_offsets() {
        let profile = FixtureProfile {
            id: "test_mover".to_string(),
            name: "Test Mover".to_string(),
            manufacturer: Some("Acme".to_string()),
            channel_count: 4,
            parameters: vec![
                param(FixtureParameter::Pan, 0),
                param(FixtureParameter::PanFine, 1),
                param(FixtureParameter::Tilt, 2),
                param(FixtureParameter::TiltFine, 3),
            ],
            notes: None,
        };
        let bytes = export_gdtf(&profile);
        let xml = read_description(&bytes);
        assert!(xml.contains("Offset=\"1,2\""));
        assert!(xml.contains("Offset=\"3,4\""));
        assert!(!xml.contains("PanFine"));
    }

    #[test]
    fn filename_uses_manufacturer_and_model() {
        let profile = FixtureProfile {
            id: "x".to_string(),
            name: "LED Par 64".to_string(),
            manufacturer: Some("Acme Co".to_string()),
            channel_count: 1,
            parameters: vec![],
            notes: None,
        };
        assert_eq!(gdtf_filename(&profile), "Acme_Co@LED_Par_64.gdtf");
    }
}
