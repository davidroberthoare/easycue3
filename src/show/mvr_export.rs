//! MVR (`.mvr`) fixture + patch export.
//!
//! Unlike USITT ASCII, MVR can carry fixture type (an embedded GDTF file),
//! DMX address, and console fixture-number in one shot — closing the patch
//! gap that GDTF-then-ASCII can't (see `gdtf_export.rs` and `ascii_export.rs`
//! docs: EOS's ASCII importer can only resolve `$Personality` blocks against
//! its own already-known fixture library, never build one from the file).
//! Recommended import order into an MVR-aware console: import the `.mvr`
//! first (patches every fixture with its real embedded GDTF personality and
//! DMX address), then the companion ASCII (`ascii_export::export_ascii` with
//! `AsciiExportMode::MvrMatch`). That ASCII has no `Clear All` and its patch
//! section references the same GDTF identities the MVR embedded, so it
//! re-patches the identical fixtures rather than resetting them, then adds
//! cue levels on top.

use crate::fixtures::profiles::FixtureProfile;
use crate::fixtures::Patch;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;

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

/// Deterministic RFC4122-formatted UUID derived from a seed string. Only
/// needs to be stable and unique within our own exported files.
fn mvr_guid(seed: &str) -> String {
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

/// Build an MVR archive (`GeneralSceneDescription.xml` plus one embedded
/// `.gdtf` per distinct patched profile) describing every patched fixture's
/// type, DMX address, and console fixture number.
pub fn export_mvr(patch: &[Patch], profiles: &HashMap<String, FixtureProfile>) -> Vec<u8> {
    let mut sorted: Vec<&Patch> = patch.iter().collect();
    sorted.sort_by_key(|p| p.id);

    let mut xml = String::with_capacity(4096);
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    writeln!(
        xml,
        "<GeneralSceneDescription verMajor=\"1\" verMinor=\"6\" provider=\"EasyCue3\" providerVersion=\"{}\">",
        env!("CARGO_PKG_VERSION")
    )
    .unwrap();
    xml.push_str("  <Scene>\n    <Layers>\n");
    writeln!(
        xml,
        "      <Layer uuid=\"{}\" name=\"EasyCue3\">",
        mvr_guid("layer")
    )
    .unwrap();
    xml.push_str("        <ChildList>\n");

    for p in &sorted {
        let Some(profile) = profiles.get(&p.profile_id) else {
            continue;
        };
        let filename = crate::show::gdtf_export::gdtf_filename(profile);
        let uuid = mvr_guid(&format!("fixture-{}", p.id));
        let label = escape_xml(if p.label.is_empty() {
            &profile.name
        } else {
            &p.label
        });
        writeln!(xml, "          <Fixture name=\"{label}\" uuid=\"{uuid}\">").unwrap();
        writeln!(xml, "            <GDTFSpec>{filename}</GDTFSpec>").unwrap();
        xml.push_str("            <GDTFMode>Default</GDTFMode>\n");
        xml.push_str("            <Addresses>\n");
        writeln!(
            xml,
            "              <Address break=\"0\">{}.{}</Address>",
            p.universe, p.start_address
        )
        .unwrap();
        xml.push_str("            </Addresses>\n");
        writeln!(xml, "            <FixtureID>{}</FixtureID>", p.id).unwrap();
        writeln!(
            xml,
            "            <FixtureIDNumeric>{}</FixtureIDNumeric>",
            p.id
        )
        .unwrap();
        xml.push_str("          </Fixture>\n");
    }

    xml.push_str("        </ChildList>\n");
    xml.push_str("      </Layer>\n");
    xml.push_str("    </Layers>\n");
    xml.push_str("  </Scene>\n");
    xml.push_str("</GeneralSceneDescription>\n");

    let mut profile_ids: Vec<&String> = sorted.iter().map(|p| &p.profile_id).collect();
    profile_ids.sort();
    profile_ids.dedup();

    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("GeneralSceneDescription.xml", options)
            .expect("failed to start GeneralSceneDescription.xml in MVR zip");
        zip.write_all(xml.as_bytes())
            .expect("failed to write GeneralSceneDescription.xml contents");

        for profile_id in &profile_ids {
            let Some(profile) = profiles.get(*profile_id) else {
                continue;
            };
            let filename = crate::show::gdtf_export::gdtf_filename(profile);
            let gdtf_bytes = crate::show::gdtf_export::export_gdtf(profile);
            zip.start_file(&filename, options)
                .expect("failed to start embedded GDTF entry in MVR zip");
            zip.write_all(&gdtf_bytes)
                .expect("failed to write embedded GDTF entry");
        }
        zip.finish().expect("failed to finalize MVR zip");
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::profiles::{FixtureParameter, ParameterMapping};
    use std::io::Read;

    fn rgb_profile() -> FixtureProfile {
        FixtureProfile {
            id: "rgb".to_string(),
            name: "RGB".to_string(),
            manufacturer: Some("Acme".to_string()),
            channel_count: 3,
            parameters: vec![
                ParameterMapping {
                    parameter: FixtureParameter::Red,
                    channel_offset: 0,
                    default_value: None,
                },
                ParameterMapping {
                    parameter: FixtureParameter::Green,
                    channel_offset: 1,
                    default_value: None,
                },
                ParameterMapping {
                    parameter: FixtureParameter::Blue,
                    channel_offset: 2,
                    default_value: None,
                },
            ],
            notes: None,
        }
    }

    fn read_zip_entry(bytes: &[u8], name: &str) -> String {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut file = archive.by_name(name).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        contents
    }

    #[test]
    fn mvr_embeds_gdtf_and_references_it_with_address_and_fixture_id() {
        let mut profiles = HashMap::new();
        profiles.insert("rgb".to_string(), rgb_profile());
        let patch = vec![Patch {
            id: 11,
            label: "SL Wash".to_string(),
            profile_id: "rgb".to_string(),
            start_address: 100,
            universe: 2,
            notes: String::new(),
        }];

        let bytes = export_mvr(&patch, &profiles);
        let scene = read_zip_entry(&bytes, "GeneralSceneDescription.xml");

        assert!(scene.contains("<GDTFSpec>Acme@RGB.gdtf</GDTFSpec>"));
        assert!(scene.contains("<Address break=\"0\">2.100</Address>"));
        assert!(scene.contains("<FixtureID>11</FixtureID>"));
        assert!(scene.contains("name=\"SL Wash\""));

        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        assert!(archive.by_name("Acme@RGB.gdtf").is_ok());
    }
}
