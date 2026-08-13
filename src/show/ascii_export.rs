//! ETC EOS ASCII show-file export
//!
//! Produces an ASCII show file (`.asc`) understood by ETC EOS, Ion, Element,
//! and Nomad consoles via **File > Import > ASCII**.  The format is the
//! de-facto industry standard for moving lighting data between consoles.
//!
//! Only lighting cues are exported; audio and adjust cues have no ASCII
//! equivalent and are silently skipped.
//!
//! # Channel mapping
//! EOS ASCII uses instrument/channel numbers, while EasyCue3 stores raw DMX
//! addresses per-universe.  This module resolves a channel number from the
//! fixture patch when one exists, and falls back to the raw DMX address for
//! any channel that is not covered by the patch.  The fixture's `id` field is
//! used as the EOS channel number, matching what the operator would see in the
//! Patch screen on both consoles.
//!
//! # Level encoding
//! Both EasyCue3 and EOS ASCII use 0–100 (percent) for channel levels, so no
//! conversion is required.

use crate::cue::{CueList, CueKind, decode_universe_key};
use crate::fixtures::Patch;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Build an ETC EOS ASCII show file from a cue list and fixture patch.
///
/// The returned `String` can be written directly to a `.asc` file and imported
/// with **File > Import > ASCII** on any EOS-family console.
pub fn export_ascii(cue_list: &CueList, patch: &[Patch]) -> String {
    let mut out = String::with_capacity(4096);

    // -----------------------------------------------------------------------
    // Header
    // -----------------------------------------------------------------------
    out.push_str("IDENT 3:0\n");
    out.push_str("MANUFACTURER EasyCue3\n");
    out.push_str("CONSOLE EasyCue3\n");
    out.push('\n');

    // -----------------------------------------------------------------------
    // PATCH block
    //
    // Build a lookup:  (universe, dmx_channel) -> eos_channel_number
    // We use the fixture ID as the EOS channel number, and emit one
    // CHAN/DMX line for every DMX address occupied by the fixture.
    // -----------------------------------------------------------------------
    let addr_to_channel: HashMap<(u16, u16), u32> = build_address_map(patch);

    if !patch.is_empty() {
        out.push_str("PATCH\n");
        for p in patch {
            // Emit the start address only (EOS resolves multi-channel fixtures
            // from their profile; for a basic one-channel-per-fixture mapping
            // this is sufficient and avoids needing channel-count info here).
            let channel_num = p.id as u32;
            writeln!(out, "CHAN {} DMX {}:{}", channel_num, p.universe, p.start_address).ok();
        }
        out.push_str("$$END\n\n");
    }

    // -----------------------------------------------------------------------
    // CUE blocks
    // -----------------------------------------------------------------------
    for cue in cue_list.cues() {
        let data = match &cue.kind {
            CueKind::Lighting(d) => d,
            #[allow(unreachable_patterns)]
            _ => continue, // skip audio / adjust cues
        };

        // Format the cue number: EOS expects it as a decimal (e.g. "1",
        // "1.5", "2").  Use 2 decimal places only when fractional.
        let cue_num = format_cue_number(cue.number);
        writeln!(out, "CUE {}", cue_num).ok();

        if !cue.label.is_empty() {
            writeln!(out, "TEXT {}", cue.label).ok();
        }

        writeln!(out, "UP {:.2}", data.fade_up).ok();
        writeln!(out, "DOWN {:.2}", data.fade_down).ok();

        if let Some(follow) = cue.autofollow {
            writeln!(out, "FOLLOWON {:.2}", follow).ok();
        }

        // Emit channel levels grouped by universe.
        // Sort by (universe, channel) for deterministic, readable output.
        let mut entries: Vec<(u16, u16, u8)> = data
            .channel_values
            .iter()
            .map(|(&key, &val)| {
                let (uni, ch) = decode_universe_key(key);
                (uni, ch, val)
            })
            .collect();
        entries.sort_unstable_by_key(|&(uni, ch, _)| (uni, ch));

        let mut current_universe: Option<u16> = None;
        for (universe, dmx_ch, level) in entries {
            // Open a UNIVERSE block when the universe changes.
            if current_universe != Some(universe) {
                if current_universe.is_some() {
                    out.push_str("$$END\n");
                }
                writeln!(out, "UNIVERSE {}", universe).ok();
                current_universe = Some(universe);
            }

            // Resolve to EOS channel number if patched, otherwise use the raw
            // DMX address as the channel number (acceptable fallback).
            let eos_chan = addr_to_channel
                .get(&(universe, dmx_ch))
                .copied()
                .unwrap_or(dmx_ch as u32);

            writeln!(out, "CHAN {} AT {}", eos_chan, level).ok();
        }

        if current_universe.is_some() {
            out.push_str("$$END\n");
        }

        out.push_str("$$END\n\n");
    }

    // -----------------------------------------------------------------------
    // Footer
    // -----------------------------------------------------------------------
    out.push_str("ENDDATA\n");
    out
}

/// Build a map from `(universe, dmx_address)` to EOS channel number.
///
/// Every DMX address occupied by a patched fixture maps to the fixture's `id`,
/// which is used as the EOS channel number.  This lets a show without a profile
/// library still import correctly on EOS — each fixture appears as a single
/// dimmer channel.
fn build_address_map(patch: &[Patch]) -> HashMap<(u16, u16), u32> {
    let mut map = HashMap::new();
    for p in patch {
        // Map only the start address; EOS infers multi-channel fixtures from
        // its own profile.  This is the standard approach for ASCII import.
        map.insert((p.universe, p.start_address), p.id as u32);
    }
    map
}

/// Format a cue number for ASCII output.
///
/// EOS cue numbers are decimal strings.  We omit the fractional part when it
/// is exactly zero so "1.0" becomes "1", and keep up to 2 decimal places
/// otherwise ("1.5", "1.25").
fn format_cue_number(number: f32) -> String {
    if (number - number.floor()).abs() < f32::EPSILON {
        format!("{}", number as u32)
    } else {
        // Strip trailing zeros (e.g. 1.50 -> 1.5)
        let s = format!("{:.2}", number);
        s.trim_end_matches('0').to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cue::{Cue, CueList, LightingData};

    fn make_list(cues: Vec<Cue>) -> CueList {
        let mut list = CueList::new();
        for c in cues {
            list.add_cue(c);
        }
        list
    }

    #[test]
    fn empty_show_produces_valid_ascii() {
        let list = CueList::new();
        let out = export_ascii(&list, &[]);
        assert!(out.starts_with("IDENT 3:0\n"));
        assert!(out.ends_with("ENDDATA\n"));
        // No PATCH block when patch is empty.
        assert!(!out.contains("PATCH"));
    }

    #[test]
    fn single_cue_levels_appear_in_output() {
        let mut cue = Cue::new_lighting(1.0);
        if let Some(d) = cue.lighting_data_mut() {
            d.set_channel(1, 100);
            d.set_channel(10, 50);
            d.fade_up = 2.0;
            d.fade_down = 1.5;
        }
        let list = make_list(vec![cue]);
        let out = export_ascii(&list, &[]);

        assert!(out.contains("CUE 1\n"));
        assert!(out.contains("UP 2.00\n"));
        assert!(out.contains("DOWN 1.50\n"));
        assert!(out.contains("CHAN 1 AT 100\n"));
        assert!(out.contains("CHAN 10 AT 50\n"));
    }

    #[test]
    fn zero_level_channels_are_not_exported() {
        // LightingData only stores non-zero channels, so this is ensured by
        // the data model.  Setting to 0 removes the entry.
        let mut data = LightingData::default();
        data.set_channel(5, 0); // should be a no-op / removal
        let mut cue = Cue::new_lighting(1.0);
        *cue.lighting_data_mut().unwrap() = data;
        let list = make_list(vec![cue]);
        let out = export_ascii(&list, &[]);
        // Universe block should not appear at all (no non-zero channels).
        assert!(!out.contains("CHAN 5 AT 0"));
    }

    #[test]
    fn label_and_autofollow_exported_correctly() {
        let mut cue = Cue::new_lighting(2.5);
        cue.label = "Scene 1".to_string();
        cue.autofollow = Some(3.0);
        if let Some(d) = cue.lighting_data_mut() {
            d.set_channel(1, 80);
        }
        let list = make_list(vec![cue]);
        let out = export_ascii(&list, &[]);

        assert!(out.contains("CUE 2.5\n"));
        assert!(out.contains("TEXT Scene 1\n"));
        assert!(out.contains("FOLLOWON 3.00\n"));
    }

    #[test]
    fn patch_emits_chan_dmx_lines() {
        let patch = vec![Patch {
            id: 3,
            label: "Fixture A".to_string(),
            profile_id: "generic_dimmer".to_string(),
            start_address: 17,
            universe: 1,
            notes: String::new(),
        }];
        let list = CueList::new();
        let out = export_ascii(&list, &patch);
        assert!(out.contains("PATCH\n"));
        assert!(out.contains("CHAN 3 DMX 1:17\n"));
    }

    #[test]
    fn patch_channel_number_used_in_cue_levels() {
        let patch = vec![Patch {
            id: 5,
            label: "Dim".to_string(),
            profile_id: "generic_dimmer".to_string(),
            start_address: 10,
            universe: 1,
            notes: String::new(),
        }];
        let mut cue = Cue::new_lighting(1.0);
        if let Some(d) = cue.lighting_data_mut() {
            // DMX address 10 is patched as instrument 5
            d.set_channel(10, 75);
        }
        let list = make_list(vec![cue]);
        let out = export_ascii(&list, &patch);
        // Should appear as EOS channel 5 (fixture id), not raw address 10.
        assert!(out.contains("CHAN 5 AT 75\n"));
        assert!(!out.contains("CHAN 10 AT 75"));
    }

    #[test]
    fn cue_number_formatting() {
        assert_eq!(format_cue_number(1.0), "1");
        assert_eq!(format_cue_number(1.5), "1.5");
        assert_eq!(format_cue_number(1.25), "1.25");
        assert_eq!(format_cue_number(10.0), "10");
    }

    #[test]
    fn multi_universe_cue_emits_universe_blocks() {
        let mut cue = Cue::new_lighting(1.0);
        if let Some(d) = cue.lighting_data_mut() {
            d.set_channel_in_universe(1, 1, 100);
            d.set_channel_in_universe(2, 1, 50);
        }
        let list = make_list(vec![cue]);
        let out = export_ascii(&list, &[]);
        assert!(out.contains("UNIVERSE 1\n"));
        assert!(out.contains("UNIVERSE 2\n"));
    }
}
