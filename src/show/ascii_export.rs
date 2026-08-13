//! ETC EOS-family ASCII show-file export
//!
//! Produces an ASCII show file (`.asc`) importable by ETC EOS, Ion, Element
//! and Nomad consoles via **File > Import > ASCII**.  The layout mirrors the
//! consoles' own export format (`Ident`/`Manufacturer`/`Console` headers, a
//! `$ParamType`/`$Personality`/`$Patch` fixture library, `$CueList`/`Cue`
//! blocks with `$$ChanMove` intensity moves and `$$Param` attribute data, and a
//! final `EndData`).
//!
//! # Channel mapping
//! EOS channels correspond to EasyCue3 fixture IDs: a fixture patched at
//! `(universe, start_address)` becomes one channel (its ID) at that DMX
//! address.  Each distinct fixture profile is exported as a custom `$Personality`
//! so EOS knows the fixture's footprint; cue levels recorded on the fixture's
//! sub-channels are mapped to the matching EOS parameters (Red/Green/Blue/
//! Amber/White/UV/…), and a 16-bit Pan/Tilt is rebuilt from its coarse+fine
//! halves.
//!
//! Fixtures whose profile cannot be mapped (unknown or `Custom` parameters)
//! fall back to a conventional dimmer patch: the start address becomes one
//! channel (the fixture ID) and any used sub-channels become their own channels
//! numbered by DMX address, so nothing is lost.
//!
//! # Level encoding
//! EOS ASCII writes levels as raw DMX bytes (`Hff` = full); EasyCue3 stores
//! 0–100, converted here with `intensity_to_dmx`.

use crate::cue::{decode_universe_key, universe_key, CueKind, CueList};
use crate::dmx::backends::intensity_to_dmx;
use crate::fixtures::profiles::{FixtureParameter, FixtureProfile};
use crate::fixtures::Patch;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// Maximum number of `<chan>@Hxx` pairs written per `$$ChanMove` line.
const CHANNELS_PER_LINE: usize = 10;
/// Maximum number of `<param>@<value>` pairs written per `$$Param` line.
const PARAMS_PER_LINE: usize = 8;
/// Personality IDs are allocated from here to avoid colliding with the
/// console's built-in fixture library.
const PERSONALITY_ID_BASE: u32 = 90000;

/// A parameter as EOS understands it: number, category and display name.
#[derive(Clone, Copy)]
struct EosParam {
    number: u32,
    category: u8,
    name: &'static str,
}

/// Map an EasyCue3 fixture parameter to the equivalent EOS parameter.
///
/// Numbers for the common colour/intensity/position/shutter parameters are
/// taken from EOS's own exports; Focus/Gobo/Prism/Frost use the standard EOS
/// numbers where known.  Unknown or `Custom` parameters return `None` so the
/// whole fixture can fall back to a flat dimmer patch.
fn eos_param(p: &FixtureParameter) -> Option<EosParam> {
    use FixtureParameter as F;
    Some(match p {
        F::Intensity => EosParam {
            number: 1,
            category: 1,
            name: "Intens",
        },
        F::Pan | F::PanFine => EosParam {
            number: 2,
            category: 2,
            name: "Pan",
        },
        F::Tilt | F::TiltFine => EosParam {
            number: 3,
            category: 2,
            name: "Tilt",
        },
        F::Red => EosParam {
            number: 12,
            category: 3,
            name: "Red",
        },
        F::Green => EosParam {
            number: 13,
            category: 3,
            name: "Green",
        },
        F::Blue => EosParam {
            number: 14,
            category: 3,
            name: "Blue",
        },
        F::Uv => EosParam {
            number: 15,
            category: 3,
            name: "UV",
        },
        F::Amber => EosParam {
            number: 48,
            category: 3,
            name: "Amber",
        },
        F::White => EosParam {
            number: 51,
            category: 3,
            name: "White",
        },
        F::Strobe => EosParam {
            number: 204,
            category: 6,
            name: "ShutterStrobe",
        },
        F::Iris => EosParam {
            number: 73,
            category: 5,
            name: "Iris",
        },
        F::Zoom => EosParam {
            number: 79,
            category: 5,
            name: "Zoom",
        },
        F::Focus => EosParam {
            number: 5,
            category: 2,
            name: "Focus",
        },
        F::Gobo => EosParam {
            number: 75,
            category: 4,
            name: "Gobo",
        },
        F::Prism => EosParam {
            number: 81,
            category: 5,
            name: "Prism",
        },
        F::Frost => EosParam {
            number: 83,
            category: 5,
            name: "Frost",
        },
        F::Custom(_) => return None,
    })
}

/// One parameter channel of an exported EOS personality.
struct PersonalityParam {
    number: u32,
    size: u8,        // 1 = 8-bit, 2 = 16-bit
    dmx_offset: u16, // 1-based offset of the MSB within the fixture
    lsb_offset: u16, // 1-based offset of the 16-bit LSB (0 for 8-bit)
    home: u32,
}

/// Build the parameter list for a profile's personality, merging Pan/PanFine
/// and Tilt/TiltFine into single 16-bit parameters.  Returns `None` if any
/// parameter cannot be mapped to an EOS parameter.
fn build_personality(profile: &FixtureProfile) -> Option<Vec<PersonalityParam>> {
    let mut out = Vec::new();
    for m in &profile.parameters {
        match &m.parameter {
            FixtureParameter::PanFine | FixtureParameter::TiltFine => continue,
            _ => {}
        }
        let eos = eos_param(&m.parameter)?;

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

        let (size, lsb_offset, home) = match fine {
            Some(f) => {
                let coarse = m.default_value.unwrap_or(0) as u32;
                let fine_v = f.default_value.unwrap_or(0) as u32;
                (2u8, f.channel_offset + 1, coarse * 256 + fine_v)
            }
            None => (1u8, 0u16, m.default_value.unwrap_or(0) as u32),
        };

        out.push(PersonalityParam {
            number: eos.number,
            size,
            dmx_offset: m.channel_offset + 1,
            lsb_offset,
            home: home.min(65535),
        });
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

/// A fixture combo from EOS's own built-in "Generic" fixture library.
///
/// EOS's ASCII import resolves personalities by matching `$$Dcid` against its
/// own already-known fixture library — it does not construct a working
/// custom personality purely from `$$PersChan` data in the file. A made-up
/// Dcid for a combo EOS doesn't already know either collapses to a plain
/// Dimmer (if it has an Intensity parameter) or is dropped entirely (if it
/// doesn't), silently losing all colour/position data. Matching one of these
/// known combos exactly (by ordered EOS parameter numbers) lets EOS resolve
/// the real library entry and keep full parameter control.
struct KnownPersonality {
    model: &'static str,
    dcid: &'static str,
}

/// EOS parameter-number sequences (in DMX-offset order) mapped to their
/// built-in `Generic` library entry, taken from a genuine ETC Eos ASCII
/// export.
const KNOWN_PERSONALITIES: &[(&[u32], KnownPersonality)] = &[
    (
        &[1],
        KnownPersonality {
            model: "Dimmer",
            dcid: "1631553D-CE8B-416F-B9C8-D8269CBA7F43",
        },
    ),
    (
        &[1, 12, 13, 14],
        KnownPersonality {
            model: "LED_IRGB_8B",
            dcid: "A3EDCB15-EC47-5A43-BFC6-3FD4812C0566",
        },
    ),
    (
        &[1, 12, 13, 14, 48],
        KnownPersonality {
            model: "LED_IRGBA_8B",
            dcid: "14322527-980C-714A-BFF3-3521E51BD6F0",
        },
    ),
    (
        &[1, 12, 13, 14, 48, 204],
        KnownPersonality {
            model: "LED_IRGBASt",
            dcid: "6522502A-DCC6-7946-BDF8-0C04B70D6F26",
        },
    ),
    (
        &[1, 12, 13, 14, 48, 51, 204],
        KnownPersonality {
            model: "LED_IRGBAWSt",
            dcid: "BCA614CC-F9D3-984C-82C7-F08175B75DA3",
        },
    ),
    (
        &[1, 12, 13, 14, 51, 204],
        KnownPersonality {
            model: "LED_IRGBWS",
            dcid: "8C5F7629-A05A-6B49-BB1D-A491A6FADCF1",
        },
    ),
    (
        &[12, 13, 14, 51],
        KnownPersonality {
            model: "LED_RGBW_8B",
            dcid: "B58B04B9-FEF8-3B4E-8332-9FC7B6AB4D26",
        },
    ),
];

/// Look up a built personality's EOS library match by its ordered parameter
/// numbers, if any.
fn known_personality(params: &[PersonalityParam]) -> Option<&'static KnownPersonality> {
    let numbers: Vec<u32> = params.iter().map(|p| p.number).collect();
    KNOWN_PERSONALITIES
        .iter()
        .find(|(sig, _)| *sig == numbers.as_slice())
        .map(|(_, known)| known)
}

/// Offset lookup for a fixture: which personality parameter (if any) owns each
/// DMX offset (0-based) inside the fixture.
enum ParamSlot {
    Msb(usize),
    Lsb(usize),
}

/// Where a cue's DMX-address value should land on the EOS side.
enum ChannelTarget {
    /// Intensity or flat channel level: `$$ChanMove <chan>@Hxx`.
    Move(u32),
    /// Single-byte attribute: `$$Param <chan> <num>@<dmx>`.
    Param8 { chan: u32, num: u32 },
    /// 16-bit attribute rebuilt from coarse+fine halves (absolute DMX addrs).
    Param16 {
        chan: u32,
        num: u32,
        msb_addr: u16,
        lsb_addr: u16,
    },
}

/// Per-fixture export info.
struct FixtureExport {
    id: u32,
    start: u16,
    universe: u16,
    label: String,
    channel_count: u16,
    personality: Option<u32>,
    params: Vec<PersonalityParam>,
    param_slots: HashMap<u16, ParamSlot>,
}

/// Channel number for an address that is not a fixture start: the address
/// itself, offset by 512 per universe beyond the first so repeated addresses
/// on different universes stay distinct.
fn flat_channel(universe: u16, address: u16) -> u32 {
    address as u32 + (universe as u32 - 1) * 512
}

/// Build the fixture export table in fixture-ID order.
fn build_fixtures(
    patch: &[Patch],
    profiles: &HashMap<String, FixtureProfile>,
) -> Vec<FixtureExport> {
    let mut sorted: Vec<&Patch> = patch.iter().collect();
    sorted.sort_by_key(|p| p.id);

    // First pass: build parameter/slot tables for fixtures whose profile maps
    // cleanly onto EOS parameters.
    let mut fixtures: Vec<FixtureExport> = Vec::with_capacity(sorted.len());
    for p in &sorted {
        let profile = profiles.get(&p.profile_id);
        let channel_count = profile.map(|pr| pr.channel_count).unwrap_or(1);
        let (params, param_slots) = match profile.and_then(build_personality) {
            // Only fixtures matching one of EOS's own built-in "Generic"
            // personalities can be patched as a real multi-parameter fixture
            // (see `known_personality`); anything else falls back to flat
            // per-address channels so its levels aren't silently dropped.
            Some(params) if known_personality(&params).is_some() => {
                let mut slots = HashMap::new();
                for (idx, param) in params.iter().enumerate() {
                    slots.insert(param.dmx_offset - 1, ParamSlot::Msb(idx));
                    if param.size == 2 {
                        slots.insert(param.lsb_offset - 1, ParamSlot::Lsb(idx));
                    }
                }
                (params, slots)
            }
            _ => (Vec::new(), HashMap::new()),
        };
        fixtures.push(FixtureExport {
            id: p.id as u32,
            start: p.start_address,
            universe: p.universe,
            label: p.label.clone(),
            channel_count,
            personality: None,
            params,
            param_slots,
        });
    }

    // Second pass: assign one personality ID per distinct mappable profile.
    let mut pid = PERSONALITY_ID_BASE;
    let mut by_profile: HashMap<String, u32> = HashMap::new();
    for (fx, p) in fixtures.iter_mut().zip(sorted.iter()) {
        if fx.params.is_empty() {
            continue;
        }
        let assigned = *by_profile.entry(p.profile_id.clone()).or_insert_with(|| {
            let id = pid;
            pid += 1;
            id
        });
        fx.personality = Some(assigned);
    }

    fixtures
}

/// Whether a DMX address needs a conventional `Patch` record, i.e. it is not
/// covered by a personality fixture parameter.
fn needs_conventional_patch(
    fixtures: &[FixtureExport],
    reserved_ids: &HashSet<u32>,
    uni: u16,
    addr: u16,
) -> bool {
    let Some(fx) = fixtures
        .iter()
        .find(|f| f.universe == uni && addr >= f.start && addr < f.start + f.channel_count)
    else {
        return true;
    };
    if fx.personality.is_none() {
        return true;
    }
    let offset = addr - fx.start;
    if fx.param_slots.contains_key(&offset) {
        // Covered by the fixture's personality — handled by its $Patch record.
        return false;
    }
    // Uncovered address: needs a flat channel, unless that channel number
    // would clash with a fixture-ID channel.
    !reserved_ids.contains(&flat_channel(uni, addr))
}

/// Resolve the channel number used by a fallback (non-personality) address.
fn resolve_fallback_channel(fixtures: &[FixtureExport], uni: u16, addr: u16) -> u32 {
    if let Some(fx) = fixtures
        .iter()
        .find(|f| f.universe == uni && addr >= f.start && addr < f.start + f.channel_count)
    {
        if fx.personality.is_none() && addr == fx.start {
            return fx.id;
        }
    }
    flat_channel(uni, addr)
}

/// Resolve where a cue value for `(uni, addr)` lands on the EOS side.
fn resolve_target(fixtures: &[FixtureExport], uni: u16, addr: u16) -> ChannelTarget {
    let Some(fx) = fixtures
        .iter()
        .find(|f| f.universe == uni && addr >= f.start && addr < f.start + f.channel_count)
    else {
        return ChannelTarget::Move(flat_channel(uni, addr));
    };
    if fx.personality.is_none() {
        let chan = if addr == fx.start {
            fx.id
        } else {
            flat_channel(uni, addr)
        };
        return ChannelTarget::Move(chan);
    }
    let offset = addr - fx.start;
    let Some(slot) = fx.param_slots.get(&offset) else {
        return ChannelTarget::Move(flat_channel(uni, addr));
    };
    let idx = match slot {
        ParamSlot::Msb(i) | ParamSlot::Lsb(i) => *i,
    };
    let p = &fx.params[idx];
    if p.number == 1 {
        ChannelTarget::Move(fx.id)
    } else if p.size == 2 {
        ChannelTarget::Param16 {
            chan: fx.id,
            num: p.number,
            msb_addr: fx.start + p.dmx_offset - 1,
            lsb_addr: fx.start + p.lsb_offset - 1,
        }
    } else {
        ChannelTarget::Param8 {
            chan: fx.id,
            num: p.number,
        }
    }
}

/// Build an ETC EOS-family ASCII show file from a cue list and fixture patch.
///
/// `profiles` maps profile IDs to their definitions (used to build fixture
/// personalities); `title` is used for the `$$Title` record and the cue-list
/// label.  Only lighting cues are exported; audio and adjust cues have no
/// ASCII equivalent and are silently skipped.
pub fn export_ascii(
    cue_list: &CueList,
    patch: &[Patch],
    profiles: &HashMap<String, FixtureProfile>,
    title: &str,
) -> String {
    let fixtures = build_fixtures(patch, profiles);
    let reserved_ids: HashSet<u32> = fixtures.iter().map(|f| f.id).collect();

    // -----------------------------------------------------------------------
    // Pre-scan the cue data: which conventional `Patch` lines are needed for
    // non-personality channels, and the highest channel number in use.
    // -----------------------------------------------------------------------
    let mut conventional_patch: Vec<(u16, u16, u32)> = Vec::new(); // (universe, addr, chan)
    for cue in cue_list.cues() {
        let CueKind::Lighting(data) = &cue.kind else {
            continue;
        };
        for &key in data.channel_values.keys() {
            let (uni, addr) = decode_universe_key(key);
            if needs_conventional_patch(&fixtures, &reserved_ids, uni, addr) {
                let chan = resolve_fallback_channel(&fixtures, uni, addr);
                conventional_patch.push((uni, addr, chan));
            }
        }
    }
    // Always patch fallback fixtures' start addresses, even if never used in a cue.
    for fx in &fixtures {
        if fx.personality.is_none() {
            let entry = (fx.universe, fx.start, fx.id);
            if !conventional_patch.contains(&entry) {
                conventional_patch.push(entry);
            }
        }
    }
    conventional_patch.sort_unstable();
    conventional_patch.dedup();

    let mut max_channel = 512u32;
    for &(_, _, chan) in &conventional_patch {
        max_channel = max_channel.max(chan);
    }
    for fx in &fixtures {
        max_channel = max_channel.max(fx.id);
    }

    let mut out = String::with_capacity(8192);

    // -----------------------------------------------------------------------
    // Header
    // -----------------------------------------------------------------------
    out.push_str("Ident 3:0\n");
    out.push_str("Manufacturer EasyCue3\n");
    out.push_str("Console EasyCue3\n");
    out.push_str("$$Format 3.20\n");
    if !title.is_empty() {
        writeln!(out, "$$Title {}", single_line(title)).ok();
    }
    out.push_str("Clear All\n");
    writeln!(out, "Set Channels {}\n", max_channel).ok();

    // -----------------------------------------------------------------------
    // Fixture parameter definitions ($ParamType)
    // -----------------------------------------------------------------------
    let mut param_numbers: Vec<u32> = fixtures
        .iter()
        .filter(|f| !f.params.is_empty())
        .flat_map(|f| f.params.iter().map(|p| p.number))
        .collect();
    param_numbers.sort_unstable();
    param_numbers.dedup();
    for num in &param_numbers {
        let Some(eos) = eos_param_by_number(*num) else {
            continue;
        };
        writeln!(out, "$ParamType {:>12} {} {}", num, eos.category, eos.name).ok();
        writeln!(out, "   $$ShortName {}", eos.name).ok();
    }

    // -----------------------------------------------------------------------
    // Fixture personalities + patch
    // -----------------------------------------------------------------------
    if !fixtures.is_empty() {
        // One $Personality per distinct personality ID (first fixture wins).
        let mut seen_pids: Vec<u32> = fixtures.iter().filter_map(|f| f.personality).collect();
        seen_pids.sort_unstable();
        seen_pids.dedup();
        for pid in &seen_pids {
            let fx = fixtures
                .iter()
                .find(|f| f.personality == Some(*pid))
                .unwrap();
            let p = patch.iter().find(|p| p.id == fx.id as usize).unwrap();
            let Some(profile) = profiles.get(&p.profile_id) else {
                continue;
            };
            // Matched against `KNOWN_PERSONALITIES` in `build_fixtures`, so
            // this always resolves for a fixture with `personality` set.
            let known = known_personality(&fx.params).unwrap();
            writeln!(out, "$Personality {}", pid).ok();
            writeln!(out, "   $$Manuf Generic").ok();
            writeln!(out, "   $$Model {}", known.model).ok();
            writeln!(out, "   $$Dcid {}", known.dcid).ok();
            writeln!(out, "   $$Footprint {}", profile.channel_count).ok();
            for p in &fx.params {
                if p.size == 2 {
                    writeln!(
                        out,
                        "   $$PersChan {:>5}     2 {:>5} {:>5} {:>5}",
                        p.number, p.dmx_offset, p.lsb_offset, p.home
                    )
                    .ok();
                    writeln!(
                        out,
                        "    $$PersSlot     0 65535 {:>5}     0.000000   100.000000 %",
                        p.home
                    )
                    .ok();
                } else {
                    writeln!(
                        out,
                        "   $$PersChan {:>5}     1 {:>5}     0 {:>5}",
                        p.number, p.dmx_offset, p.home
                    )
                    .ok();
                    writeln!(
                        out,
                        "    $$PersSlot     0   255 {:>5}     0.000000   100.000000 %",
                        p.home
                    )
                    .ok();
                }
            }
            // RGB-only fixtures have no dedicated Intensity channel; without
            // this flag EOS rejects the personality (and its patch) outright.
            if !profile.has_intensity() {
                out.push_str("   $$VirtualInt \n");
            }
            out.push('\n');
        }

        for fx in &fixtures {
            if let Some(pid) = fx.personality {
                let edmx = fx.start as u32 + (fx.universe as u32 - 1) * 512;
                let mode = if edmx > 512 { 1 } else { 0 };
                let model = known_personality(&fx.params).map(|k| k.model).unwrap_or("Fixture");
                writeln!(out, "$Patch {} {} {} {} 1", fx.id, pid, edmx, mode).ok();
                writeln!(out, "   $$Pers {}", single_line(model)).ok();
                if !fx.label.is_empty() {
                    writeln!(out, "   Text {}", single_line(&fx.label)).ok();
                }
            } else {
                writeln!(out, "Patch {} {}<{}@Hff", fx.universe, fx.id, fx.start).ok();
            }
        }
        out.push('\n');
    }

    // Conventional patch for non-personality channels (fallback sub-channels,
    // personality fixtures' uncovered addresses, and addresses outside any
    // fixture).  Skip entries that were already emitted as a fixture's own
    // Patch line above.
    for (uni, addr, chan) in &conventional_patch {
        let is_own_patch = fixtures.iter().any(|f| {
            f.personality.is_none() && f.id == *chan && f.start == *addr && f.universe == *uni
        });
        if is_own_patch {
            continue;
        }
        writeln!(out, "Patch {} {}<{}@Hff", uni, chan, addr).ok();
    }
    if !conventional_patch.is_empty() {
        out.push('\n');
    }

    // -----------------------------------------------------------------------
    // Cue list block
    // -----------------------------------------------------------------------
    out.push_str("$CueList 1\n");
    if !title.is_empty() {
        writeln!(out, "   Text {}", single_line(title)).ok();
    }
    out.push('\n');

    let cues = cue_list.cues();
    for (idx, cue) in cues.iter().enumerate() {
        let data = match &cue.kind {
            CueKind::Lighting(d) => d,
            #[allow(unreachable_patterns)]
            _ => continue,
        };

        writeln!(out, "Cue {} 1", format_cue_number(cue.number)).ok();
        if !cue.label.is_empty() {
            writeln!(out, "   Text {}", single_line(&cue.label)).ok();
        }
        writeln!(out, "   Up {}", format_time(data.fade_up)).ok();
        writeln!(out, "   Down {}", format_time(data.fade_down)).ok();
        if let Some(follow) = cue.autofollow {
            writeln!(out, "   $$Follow {}", format_time(follow)).ok();
        }

        // Tracked state before this cue, used to rebuild 16-bit parameters
        // when only one half changes in this cue.
        let prev_state = if idx == 0 {
            HashMap::new()
        } else {
            cue_list.tracked_state_up_to(idx - 1)
        };

        let mut moves: HashMap<u32, u8> = HashMap::new();
        let mut params: HashMap<(u32, u32), u32> = HashMap::new();

        for (&key, &level) in &data.channel_values {
            let (uni, addr) = decode_universe_key(key);
            match resolve_target(&fixtures, uni, addr) {
                ChannelTarget::Move(chan) => {
                    moves.insert(chan, level);
                }
                ChannelTarget::Param8 { chan, num } => {
                    params.insert((chan, num), intensity_to_dmx(level) as u32);
                }
                ChannelTarget::Param16 {
                    chan,
                    num,
                    msb_addr,
                    lsb_addr,
                } => {
                    let msb_key = universe_key(uni, msb_addr);
                    let lsb_key = universe_key(uni, lsb_addr);
                    let msb = if msb_addr == addr {
                        level
                    } else {
                        data.channel_values
                            .get(&msb_key)
                            .copied()
                            .or_else(|| prev_state.get(&msb_key).copied())
                            .unwrap_or(0)
                    };
                    let lsb = if lsb_addr == addr {
                        level
                    } else {
                        data.channel_values
                            .get(&lsb_key)
                            .copied()
                            .or_else(|| prev_state.get(&lsb_key).copied())
                            .unwrap_or(0)
                    };
                    params.insert(
                        (chan, num),
                        intensity_to_dmx(msb) as u32 * 256 + intensity_to_dmx(lsb) as u32,
                    );
                }
            }
        }

        // $$ChanMove lines (intensity + flat channels), sorted by channel.
        let mut move_items: Vec<(u32, u8)> = moves.into_iter().collect();
        move_items.sort_unstable_by_key(|&(chan, _)| chan);
        for chunk in move_items.chunks(CHANNELS_PER_LINE) {
            let mut line = String::from("   $$ChanMove ");
            for (i, (chan, level)) in chunk.iter().enumerate() {
                if i > 0 {
                    line.push(' ');
                }
                write!(line, "{}@H{:02x}", chan, intensity_to_dmx(*level)).ok();
            }
            out.push_str(&line);
            out.push('\n');
        }

        // `Chan` lines: EOS's recorded (tracked) levels for the cue, i.e. the
        // move channels that end up above zero. Without this record EOS
        // imports the cue but with no channel-level data.
        let chan_items: Vec<(u32, u8)> = move_items
            .iter()
            .copied()
            .filter(|&(_, level)| intensity_to_dmx(level) != 0)
            .collect();
        for chunk in chan_items.chunks(CHANNELS_PER_LINE) {
            let mut line = String::from("   Chan ");
            for (i, (chan, level)) in chunk.iter().enumerate() {
                if i > 0 {
                    line.push(' ');
                }
                write!(line, "{}@H{:02x}", chan, intensity_to_dmx(*level)).ok();
            }
            out.push_str(&line);
            out.push('\n');
        }

        // $$Param lines, grouped by channel, sorted by (channel, parameter).
        let mut param_items: Vec<((u32, u32), u32)> = params.into_iter().collect();
        param_items.sort_unstable_by_key(|&((chan, num), _)| (chan, num));
        let mut current_chan: Option<u32> = None;
        let mut line = String::new();
        let mut count = 0usize;
        for ((chan, num), val) in param_items {
            if current_chan != Some(chan) || count >= PARAMS_PER_LINE {
                if !line.is_empty() {
                    out.push_str(&line);
                    out.push('\n');
                }
                line = format!("   $$Param {} ", chan);
                current_chan = Some(chan);
                count = 0;
            } else {
                line.push(' ');
            }
            write!(line, "{}@{}", num, val).ok();
            count += 1;
        }
        if !line.is_empty() {
            out.push_str(&line);
            out.push('\n');
        }

        out.push('\n');
    }

    out.push_str("EndData\n");
    out
}

/// Look up the EOS parameter metadata by number (for `$ParamType` records).
fn eos_param_by_number(number: u32) -> Option<EosParam> {
    use FixtureParameter as F;
    let all = [
        (F::Intensity, 1u32),
        (F::Pan, 2),
        (F::Tilt, 3),
        (F::Focus, 5),
        (F::Red, 12),
        (F::Green, 13),
        (F::Blue, 14),
        (F::Uv, 15),
        (F::Amber, 48),
        (F::White, 51),
        (F::Iris, 73),
        (F::Gobo, 75),
        (F::Zoom, 79),
        (F::Prism, 81),
        (F::Frost, 83),
        (F::Strobe, 204),
    ];
    all.iter()
        .find(|(_, n)| *n == number)
        .and_then(|(p, _)| eos_param(p))
}

/// Collapse newlines — TEXT is a single-line field in the ASCII format.
fn single_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}

/// Format a fade/follow time: whole seconds without a trailing ".0".
fn format_time(t: f32) -> String {
    let s = format!("{:.2}", t);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Format a cue number for ASCII output.
///
/// EOS cue numbers are decimal strings.  We omit the fractional part when it
/// is exactly zero so "1.0" becomes "1", and keep up to 2 decimal places
/// otherwise ("1.5", "1.25").
fn format_cue_number(number: f32) -> String {
    if (number - number.floor()).abs() < 1e-4 {
        format!("{}", number as u32)
    } else {
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
    use crate::fixtures::profiles::ParameterMapping;

    fn make_list(cues: Vec<Cue>) -> CueList {
        let mut list = CueList::new();
        for c in cues {
            list.add_cue(c);
        }
        list
    }

    fn dimmer_profile() -> FixtureProfile {
        FixtureProfile {
            id: "dimmer".into(),
            name: "Dimmer".into(),
            manufacturer: None,
            channel_count: 1,
            parameters: vec![ParameterMapping {
                parameter: FixtureParameter::Intensity,
                channel_offset: 0,
                default_value: Some(0),
            }],
            notes: None,
        }
    }

    fn rgb_profile() -> FixtureProfile {
        FixtureProfile {
            id: "rgb".into(),
            name: "RGB".into(),
            manufacturer: None,
            channel_count: 3,
            parameters: vec![
                ParameterMapping {
                    parameter: FixtureParameter::Red,
                    channel_offset: 0,
                    default_value: Some(0),
                },
                ParameterMapping {
                    parameter: FixtureParameter::Green,
                    channel_offset: 1,
                    default_value: Some(0),
                },
                ParameterMapping {
                    parameter: FixtureParameter::Blue,
                    channel_offset: 2,
                    default_value: Some(0),
                },
            ],
            notes: None,
        }
    }

    fn patch(id: usize, start: u16, universe: u16, profile_id: &str) -> Patch {
        Patch {
            id,
            label: format!("Fixture {}", id),
            profile_id: profile_id.to_string(),
            start_address: start,
            universe,
            notes: String::new(),
        }
    }

    #[test]
    fn empty_show_produces_valid_ascii() {
        let list = CueList::new();
        let out = export_ascii(&list, &[], &HashMap::new(), "");
        assert!(out.starts_with("Ident 3:0\n"));
        assert!(out.ends_with("EndData\n"));
        assert!(out.contains("$CueList 1\n"));
    }

    #[test]
    fn dimmer_exports_personality_and_chanmove() {
        let mut profiles = HashMap::new();
        profiles.insert("dimmer".into(), dimmer_profile());
        let patch = vec![patch(5, 10, 1, "dimmer")];

        let mut cue = Cue::new_lighting(1.0);
        cue.lighting_data_mut().unwrap().set_channel(10, 75);
        let list = make_list(vec![cue]);
        let out = export_ascii(&list, &patch, &profiles, "");
        // Fixture 5 at address 10, one-channel personality, no flat patch.
        assert!(out.contains("$Patch 5 90000 10 0 1\n"));
        assert!(out.contains("$$PersChan     1     1     1     0     0\n"));
        assert!(!out.contains("Patch 1 "));
        // Level 75% -> DMX 0xbf, exported on the fixture's channel.
        assert!(out.contains("$$ChanMove 5@Hbf"));
    }

    #[test]
    fn rgb_fixture_falls_back_to_flat_channels() {
        // RGB-only (no Intensity) doesn't match any of EOS's built-in
        // "Generic" combos, so it must fall back to flat per-address
        // channels rather than an unrecognized custom personality.
        let mut profiles = HashMap::new();
        profiles.insert("rgb".into(), rgb_profile());
        let patch = vec![patch(11, 100, 1, "rgb")];

        let mut cue = Cue::new_lighting(1.0);
        let d = cue.lighting_data_mut().unwrap();
        d.set_channel(100, 100); // red
        d.set_channel(101, 50); // green
        d.channel_values.insert(universe_key(1, 102), 0); // blue -> explicit zero
        let list = make_list(vec![cue]);
        let out = export_ascii(&list, &patch, &profiles, "");

        assert!(!out.contains("$Personality"));
        // Each address becomes its own flat channel: fixture ID for the
        // start address, DMX address for the rest.
        assert!(out.contains("$$ChanMove 11@Hff 101@H80 102@H00"));
    }

    #[test]
    fn irgb_fixture_matches_known_eos_personality() {
        // Intensity+Red+Green+Blue matches EOS's built-in "LED_IRGB_8B",
        // so it should export as a real recognized personality.
        let profile = FixtureProfile {
            id: "irgb".into(),
            name: "iRGB".into(),
            manufacturer: None,
            channel_count: 4,
            parameters: vec![
                ParameterMapping {
                    parameter: FixtureParameter::Intensity,
                    channel_offset: 0,
                    default_value: Some(0),
                },
                ParameterMapping {
                    parameter: FixtureParameter::Red,
                    channel_offset: 1,
                    default_value: Some(0),
                },
                ParameterMapping {
                    parameter: FixtureParameter::Green,
                    channel_offset: 2,
                    default_value: Some(0),
                },
                ParameterMapping {
                    parameter: FixtureParameter::Blue,
                    channel_offset: 3,
                    default_value: Some(0),
                },
            ],
            notes: None,
        };
        let mut profiles = HashMap::new();
        profiles.insert("irgb".into(), profile);
        let patch = vec![patch(6, 20, 1, "irgb")];
        let list = make_list(vec![Cue::new_lighting(1.0)]);
        let out = export_ascii(&list, &patch, &profiles, "");

        assert!(out.contains("$$Manuf Generic\n"));
        assert!(out.contains("$$Model LED_IRGB_8B\n"));
        assert!(out.contains("$$Dcid A3EDCB15-EC47-5A43-BFC6-3FD4812C0566\n"));
        assert!(out.contains("$$Pers LED_IRGB_8B\n"));
    }

    #[test]
    fn moving_head_falls_back_to_flat_channels() {
        // Pan/Tilt + Intensity + Red isn't one of EOS's built-in "Generic"
        // combos either, so it also falls back to flat per-address channels.
        let profile = FixtureProfile {
            id: "mover".into(),
            name: "Mover".into(),
            manufacturer: None,
            channel_count: 16,
            parameters: vec![
                ParameterMapping {
                    parameter: FixtureParameter::Pan,
                    channel_offset: 0,
                    default_value: Some(127),
                },
                ParameterMapping {
                    parameter: FixtureParameter::PanFine,
                    channel_offset: 1,
                    default_value: Some(0),
                },
                ParameterMapping {
                    parameter: FixtureParameter::Tilt,
                    channel_offset: 2,
                    default_value: Some(127),
                },
                ParameterMapping {
                    parameter: FixtureParameter::TiltFine,
                    channel_offset: 3,
                    default_value: Some(0),
                },
                ParameterMapping {
                    parameter: FixtureParameter::Intensity,
                    channel_offset: 4,
                    default_value: Some(0),
                },
                ParameterMapping {
                    parameter: FixtureParameter::Red,
                    channel_offset: 5,
                    default_value: Some(255),
                },
            ],
            notes: None,
        };
        let mut profiles = HashMap::new();
        profiles.insert("mover".into(), profile);
        let patch = vec![patch(1, 1, 1, "mover")];

        let mut cue = Cue::new_lighting(1.0);
        let d = cue.lighting_data_mut().unwrap();
        d.set_channel(1, 50); // pan coarse -> fixture's own channel
        d.channel_values.insert(universe_key(1, 2), 0); // pan fine -> flat channel 2, explicit zero
        d.set_channel(6, 100); // red -> flat channel 6
        let list = make_list(vec![cue]);
        let out = export_ascii(&list, &patch, &profiles, "");

        assert!(!out.contains("$Personality"));
        assert!(out.contains("$$ChanMove 1@H80 2@H00 6@Hff"));
    }

    #[test]
    fn fallback_profile_uses_flat_patch() {
        // Profile with a Custom parameter cannot be exported as a personality.
        let profile = FixtureProfile {
            id: "custom".into(),
            name: "Custom".into(),
            manufacturer: None,
            channel_count: 2,
            parameters: vec![
                ParameterMapping {
                    parameter: FixtureParameter::Custom("thing".into()),
                    channel_offset: 0,
                    default_value: Some(0),
                },
                ParameterMapping {
                    parameter: FixtureParameter::Intensity,
                    channel_offset: 1,
                    default_value: Some(0),
                },
            ],
            notes: None,
        };
        let mut profiles = HashMap::new();
        profiles.insert("custom".into(), profile);
        let patch = vec![patch(7, 20, 1, "custom")];

        let mut cue = Cue::new_lighting(1.0);
        cue.lighting_data_mut().unwrap().set_channel(20, 80);
        let list = make_list(vec![cue]);
        let out = export_ascii(&list, &patch, &profiles, "");

        // No personality; start address becomes the fixture-ID channel.
        assert!(!out.contains("$Personality"));
        assert!(out.contains("Patch 1 7<20@Hff\n"));
        assert!(out.contains("$$ChanMove 7@Hcc"));
    }

    #[test]
    fn multi_universe_addresses_stay_distinct() {
        let mut cue = Cue::new_lighting(1.0);
        let d = cue.lighting_data_mut().unwrap();
        d.set_channel_in_universe(1, 1, 100);
        d.set_channel_in_universe(2, 1, 50);
        let list = make_list(vec![cue]);
        let out = export_ascii(&list, &[], &HashMap::new(), "");
        // No fixtures: both addresses become flat channels.
        assert!(out.contains("Patch 1 1<1@Hff\n"));
        assert!(out.contains("Patch 2 513<1@Hff\n"));
        assert!(out.contains("$$ChanMove 1@Hff 513@H80"));
    }

    #[test]
    fn zero_level_channels_are_exported_as_moves() {
        // LightingData can store an explicit 0 for a channel turning off.
        let mut data = LightingData::default();
        data.channel_values.insert(universe_key(1, 5), 0);
        let mut cue = Cue::new_lighting(1.0);
        *cue.lighting_data_mut().unwrap() = data;
        let list = make_list(vec![cue]);
        let out = export_ascii(&list, &[], &HashMap::new(), "");
        assert!(out.contains("$$ChanMove 5@H00"));
    }

    #[test]
    fn cue_number_formatting() {
        assert_eq!(format_cue_number(1.0), "1");
        assert_eq!(format_cue_number(1.5), "1.5");
        assert_eq!(format_cue_number(1.25), "1.25");
        assert_eq!(format_cue_number(10.0), "10");
    }
}
