//! Command-line parser and executor for ETC EOS-style syntax
//!
//! Supports commands like:
//! - "4a33" = channel 4 at 33%
//! - "1thru10a50" = channels 1-10 at 50%
//! - "1+3+5a75" = channels 1, 3, 5 at 75%
//! - "4" = select channel 4
//! - "1thru10" = select channels 1-10

use anyhow::{Result, bail};
use std::collections::HashSet;

/// Command context determines how hotkeys are interpreted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandContext {
    #[default]
    General,
    Lighting,
    #[allow(dead_code)]
    Sound,
}

/// Parsed command ready for execution
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Set channels to a specific level
    SetChannelLevel { channels: Vec<u16>, level: u8 },
    /// Set currently selected channels to a specific level
    SetSelectedLevel { level: u8 },
    /// Select channels (for subsequent operations)
    SelectChannels { channels: Vec<u16> },
    /// Set fixtures to a specific intensity
    SetFixtureIntensity { fixtures: Vec<usize>, intensity: f32 },
    /// Select fixtures (for subsequent operations)
    SelectFixtures { fixtures: Vec<usize> },
    /// Clear command line
    Clear,
    /// Invalid/unparseable command
    #[allow(dead_code)]
    Invalid(String),
}

/// Parse a lighting command string with context awareness
/// 
/// Syntax:
/// - Numbers: channel or fixture selection (depends on context)
/// - "a" or "@": "at level" operator
/// - "thru" or "-": range operator
/// - "+" or ",": addition operator
/// 
/// Examples:
/// - "4a33" -> Channel 4 at 33% (channel context) OR Fixture 4 at 33% (fixture context)
/// - "a50" -> Set selected channels/fixtures to 50%
/// - "4" -> Select channel 4 OR fixture 4
/// - "1thru10" -> Select channels 1-10 OR fixtures 1-10
#[allow(dead_code)]
pub fn parse_lighting_command(input: &str) -> Result<Command> {
    parse_lighting_command_with_context(input, CommandContext::General)
}

/// Parse a lighting command string with explicit context
pub fn parse_lighting_command_with_context(input: &str, context: CommandContext) -> Result<Command> {
    let input = input.trim().to_lowercase();
    
    if input.is_empty() {
        return Ok(Command::Clear);
    }
    
    // Split on "a" or "@" to separate selection from level
    let parts: Vec<&str> = if input.contains('a') {
        input.splitn(2, 'a').collect()
    } else if input.contains('@') {
        input.splitn(2, '@').collect()
    } else {
        // No level specified, just selection
        return match context {
            CommandContext::General => Ok(Command::SelectChannels {
                channels: parse_channel_selection(input.as_str())?,
            }),
            CommandContext::Lighting => Ok(Command::SelectFixtures {
                fixtures: parse_fixture_selection(input.as_str())?,
            }),
            CommandContext::Sound => Ok(Command::SelectChannels {
                channels: parse_channel_selection(input.as_str())?,
            }),
        };
    };
    
    if parts.len() != 2 {
        bail!("Invalid syntax: expected 'selection @ level'");
    }
    
    // Check if selection part is empty (e.g., "a50" with no selection)
    if parts[0].trim().is_empty() {
        // Set level on currently selected items
        let level = parse_level(parts[1])?;
        return Ok(Command::SetSelectedLevel { level });
    }
    
    // Parse level first to determine if it's percentage (0-100) or intensity (0.0-1.0)
    let level_str = parts[1];
    let level = parse_level(level_str)?;
    
    // Context-aware parsing of selection
    match context {
        CommandContext::General => {
            let channels = parse_channel_selection(parts[0])?;
            Ok(Command::SetChannelLevel { channels, level })
        }
        CommandContext::Lighting => {
            let fixtures = parse_fixture_selection(parts[0])?;
            let intensity = level as f32 / 100.0; // Convert percentage to 0.0-1.0
            Ok(Command::SetFixtureIntensity { fixtures, intensity })
        }
        CommandContext::Sound => {
            let channels = parse_channel_selection(parts[0])?;
            Ok(Command::SetChannelLevel { channels, level })
        }
    }
}

/// Parse channel selection like "1", "1thru10", "1t10", "1+3+5", "1-5+7+9-12"
fn parse_channel_selection(input: &str) -> Result<Vec<u16>> {
    let input = input.trim();
    
    if input.is_empty() {
        bail!("No channels specified");
    }
    
    let mut channels = HashSet::new();
    
    // Split on "+" or ","
    let groups: Vec<&str> = input
        .split(&['+', ','][..])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    
    for group in groups {
        // Check if it's a range (contains "thru"/"t" or "-")
        if group.contains("thru") || group.contains('t') {
            let range_parts: Vec<&str> = if group.contains("thru") {
                group.split("thru").collect()
            } else {
                group.split('t').collect()
            };
            if range_parts.len() != 2 {
                bail!("Invalid range syntax: {}", group);
            }
            let start: u16 = range_parts[0].trim().parse()
                .map_err(|_| anyhow::anyhow!("Invalid start channel: {}", range_parts[0]))?;
            let end: u16 = range_parts[1].trim().parse()
                .map_err(|_| anyhow::anyhow!("Invalid end channel: {}", range_parts[1]))?;
            
            if start < 1 || start > 512 || end < 1 || end > 512 {
                bail!("Channel must be between 1 and 512");
            }
            if start > end {
                bail!("Range start must be <= end");
            }
            
            for ch in start..=end {
                channels.insert(ch);
            }
        } else if group.contains('-') {
            // Check if it's a range with hyphen (but not negative number)
            let range_parts: Vec<&str> = group.split('-').filter(|s| !s.is_empty()).collect();
            if range_parts.len() == 2 {
                let start: u16 = range_parts[0].trim().parse()
                    .map_err(|_| anyhow::anyhow!("Invalid start channel: {}", range_parts[0]))?;
                let end: u16 = range_parts[1].trim().parse()
                    .map_err(|_| anyhow::anyhow!("Invalid end channel: {}", range_parts[1]))?;
                
                if start < 1 || start > 512 || end < 1 || end > 512 {
                    bail!("Channel must be between 1 and 512");
                }
                if start > end {
                    bail!("Range start must be <= end");
                }
                
                for ch in start..=end {
                    channels.insert(ch);
                }
            } else {
                // Single number
                let ch: u16 = group.trim().parse()
                    .map_err(|_| anyhow::anyhow!("Invalid channel: {}", group))?;
                if ch < 1 || ch > 512 {
                    bail!("Channel must be between 1 and 512");
                }
                channels.insert(ch);
            }
        } else {
            // Single channel
            let ch: u16 = group.trim().parse()
                .map_err(|_| anyhow::anyhow!("Invalid channel: {}", group))?;
            if ch < 1 || ch > 512 {
                bail!("Channel must be between 1 and 512");
            }
            channels.insert(ch);
        }
    }
    
    if channels.is_empty() {
        bail!("No valid channels specified");
    }
    
    let mut result: Vec<u16> = channels.into_iter().collect();
    result.sort_unstable();
    Ok(result)
}

/// Parse level value (0-100 percent, or 0-255 DMX)
fn parse_level(input: &str) -> Result<u8> {
    let input = input.trim();
    
    // Handle special keywords
    match input {
        "full" | "fl" | "f" => return Ok(100),
        "out" | "o" => return Ok(0),
        _ => {}
    }
    
    let value: f32 = input.parse()
        .map_err(|_| anyhow::anyhow!("Invalid level: {}", input))?;
    
    // If value is <= 100, treat as percentage
    // If value is > 100, treat as DMX value (0-255)
    let level = if value <= 100.0 {
        value as u8
    } else if value <= 255.0 {
        // Convert DMX to percentage: (value / 255) * 100
        ((value / 255.0) * 100.0) as u8
    } else {
        bail!("Level must be 0-100 (percentage) or 0-255 (DMX)");
    };
    
    Ok(level.min(100))
}

/// Parse fixture selection like "1", "1thru10", "1t10", "1+3+5", "1-5+7+9-12"
/// Similar to channel selection but returns fixture IDs (usize)
fn parse_fixture_selection(input: &str) -> Result<Vec<usize>> {
    let input = input.trim();
    
    if input.is_empty() {
        bail!("No fixtures specified");
    }
    
    let mut fixtures = HashSet::new();
    
    // Split on "+" or ","
    let groups: Vec<&str> = input
        .split(&['+', ','][..])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    
    for group in groups {
        // Check if it's a range (contains "thru"/"t" or "-")
        if group.contains("thru") || group.contains('t') {
            let range_parts: Vec<&str> = if group.contains("thru") {
                group.split("thru").collect()
            } else {
                group.split('t').collect()
            };
            if range_parts.len() != 2 {
                bail!("Invalid range syntax: {}", group);
            }
            let start: usize = range_parts[0].trim().parse()
                .map_err(|_| anyhow::anyhow!("Invalid start fixture: {}", range_parts[0]))?;
            let end: usize = range_parts[1].trim().parse()
                .map_err(|_| anyhow::anyhow!("Invalid end fixture: {}", range_parts[1]))?;
            
            if start < 1 || end < 1 {
                bail!("Fixture ID must be >= 1");
            }
            if start > end {
                bail!("Range start must be <= end");
            }
            
            for fixture_id in start..=end {
                fixtures.insert(fixture_id);
            }
        } else if group.contains('-') {
            // Check if it's a range with hyphen (but not negative number)
            let range_parts: Vec<&str> = group.split('-').filter(|s| !s.is_empty()).collect();
            if range_parts.len() == 2 {
                let start: usize = range_parts[0].trim().parse()
                    .map_err(|_| anyhow::anyhow!("Invalid start fixture: {}", range_parts[0]))?;
                let end: usize = range_parts[1].trim().parse()
                    .map_err(|_| anyhow::anyhow!("Invalid end fixture: {}", range_parts[1]))?;
                
                if start < 1 || end < 1 {
                    bail!("Fixture ID must be >= 1");
                }
                if start > end {
                    bail!("Range start must be <= end");
                }
                
                for fixture_id in start..=end {
                    fixtures.insert(fixture_id);
                }
            } else {
                // Single number
                let fixture_id: usize = group.trim().parse()
                    .map_err(|_| anyhow::anyhow!("Invalid fixture: {}", group))?;
                if fixture_id < 1 {
                    bail!("Fixture ID must be >= 1");
                }
                fixtures.insert(fixture_id);
            }
        } else {
            // Single fixture
            let fixture_id: usize = group.trim().parse()
                .map_err(|_| anyhow::anyhow!("Invalid fixture: {}", group))?;
            if fixture_id < 1 {
                bail!("Fixture ID must be >= 1");
            }
            fixtures.insert(fixture_id);
        }
    }
    
    if fixtures.is_empty() {
        bail!("No valid fixtures specified");
    }
    
    let mut result: Vec<usize> = fixtures.into_iter().collect();
    result.sort_unstable();
    Ok(result)
}

/// Execute a parsed command
pub fn execute_command(cmd: Command, app: &mut crate::app::EasyCueApp) {
    match cmd {
        Command::SetFixtureIntensity { fixtures, intensity } => {
            let mut error_count = 0;
            
            for &fixture_id in &fixtures {
                // Find the patch for this fixture
                if let Some(patch) = app.fixtures.patch_list().get_patch(fixture_id) {
                    let patch_clone = patch.clone();
                    // Get the fixture profile to determine if it has intensity channel
                    if let Some(profile) = app.fixtures.get_profile(&patch.profile_id) {
                        let profile_clone = profile.clone();
                        if let Some(universe) = app.universes.first_mut() {
                            if profile.has_intensity() {
                                // iRGB fixture: Set intensity channel directly
                                if let Some(intensity_param) = profile.parameters.iter()
                                    .find(|p| p.parameter == crate::fixtures::profiles::FixtureParameter::Intensity) 
                                {
                                    let channel = patch.start_address + intensity_param.channel_offset;
                                    let dmx_value = (intensity * 100.0).round() as u8;
                                    if let Err(e) = universe.set_channel(channel, dmx_value) {
                                        log::error!("Failed to set fixture {} intensity channel {}: {}", fixture_id, channel, e);
                                        error_count += 1;
                                    }
                                }
                            } else if profile.is_rgb() {
                                // RGB fixture: Use virtual intensity
                                if let Err(e) = app.virtual_intensity.set_intensity(
                                    fixture_id, 
                                    intensity, 
                                    universe,
                                    &patch_clone,
                                    &profile_clone,
                                ) {
                                    log::error!("Failed to set fixture {} virtual intensity: {}", fixture_id, e);
                                    error_count += 1;
                                }
                            }
                        }
                    } else {
                        log::error!("Fixture {} profile '{}' not found", fixture_id, patch.profile_id);
                        error_count += 1;
                    }
                } else {
                    log::error!("Fixture {} not found in patch list", fixture_id);
                    error_count += 1;
                }
            }
            
            // Update UI state
            app.ui_state.selected_fixtures.clear();
            for &fixture_id in &fixtures {
                app.ui_state.selected_fixtures.insert(fixture_id);
            }
            
            let fixture_list = format_fixture_list(&fixtures);
            let intensity_percent = (intensity * 100.0).round() as u8;
            if error_count == 0 {
                app.ui_state.status_message = format!("Fixture {} @ {}%", fixture_list, intensity_percent);
                log::info!("Set fixture {} to {}%", fixture_list, intensity_percent);
            } else {
                app.ui_state.status_message = format!("Fixture {} @ {}% ({} errors)", fixture_list, intensity_percent, error_count);
                log::warn!("Set fixture {} to {}% with {} errors", fixture_list, intensity_percent, error_count);
            }
        }
        Command::SelectFixtures { fixtures } => {
            app.ui_state.selected_fixtures.clear();
            for &fixture_id in &fixtures {
                app.ui_state.selected_fixtures.insert(fixture_id);
            }
            
            let fixture_list = format_fixture_list(&fixtures);
            app.ui_state.status_message = format!("Selected fixture: {}", fixture_list);
            log::info!("Selected fixtures: {}", fixture_list);
        }
        Command::SetChannelLevel { channels, level } => {
            if let Some(universe) = app.universes.first_mut() {
                for ch in &channels {
                    if let Err(e) = universe.set_channel(*ch, level) {
                        log::error!("Failed to set channel {}: {}", ch, e);
                    }
                }
                
                // Select the channels that were set
                app.ui_state.selected_channels.clear();
                app.ui_state.channel_base_levels.clear();
                for &ch in &channels {
                    app.ui_state.selected_channels.insert(ch);
                    app.ui_state.channel_base_levels.insert(ch, level);
                }
                app.ui_state.group_master = level;
                if let Some(&last_ch) = channels.last() {
                    app.ui_state.last_selected_channel = Some(last_ch);
                }
                
                let ch_list = format_channel_list(&channels);
                app.ui_state.status_message = format!("{} @ {}%", ch_list, level);
                log::info!("Set {} to {}%", ch_list, level);
            }
        }
        Command::SetSelectedLevel { level } => {
            if app.ui_state.selected_channels.is_empty() {
                app.ui_state.status_message = "No channels selected".to_string();
                log::warn!("Attempted to set level with no channels selected");
                return;
            }
            
            if let Some(universe) = app.universes.first_mut() {
                let channels: Vec<u16> = app.ui_state.selected_channels.iter().copied().collect();
                for ch in &channels {
                    if let Err(e) = universe.set_channel(*ch, level) {
                        log::error!("Failed to set channel {}: {}", ch, e);
                    }
                }
                
                let ch_list = format_channel_list(&channels);
                app.ui_state.status_message = format!("{} @ {}%", ch_list, level);
                log::info!("Set {} to {}%", ch_list, level);
            }
        }
        Command::SelectChannels { channels } => {
            app.ui_state.selected_channels.clear();
            for ch in &channels {
                app.ui_state.selected_channels.insert(*ch);
            }
            
            // Update base levels for selected channels
            if let Some(universe) = app.universes.first() {
                app.ui_state.channel_base_levels.clear();
                for &ch in &channels {
                    if let Ok(level) = universe.get_channel(ch) {
                        app.ui_state.channel_base_levels.insert(ch, level);
                    }
                }
            }
            
            let ch_list = format_channel_list(&channels);
            app.ui_state.status_message = format!("Selected: {}", ch_list);
            log::info!("Selected channels: {}", ch_list);
        }
        Command::Clear => {
            app.ui_state.status_message = "Command cleared".to_string();
        }
        Command::Invalid(msg) => {
            app.ui_state.status_message = format!("Error: {}", msg);
            log::warn!("Invalid command: {}", msg);
        }
    }
}

/// Format a channel list for display (e.g., "1-5, 7, 9-12")
fn format_channel_list(channels: &[u16]) -> String {
    if channels.is_empty() {
        return String::new();
    }
    
    let mut result = Vec::new();
    let mut range_start = channels[0];
    let mut range_end = channels[0];
    
    for i in 1..channels.len() {
        if channels[i] == range_end + 1 {
            range_end = channels[i];
        } else {
            // End of range, add to result
            if range_start == range_end {
                result.push(format!("{}", range_start));
            } else if range_end == range_start + 1 {
                result.push(format!("{}, {}", range_start, range_end));
            } else {
                result.push(format!("{}-{}", range_start, range_end));
            }
            range_start = channels[i];
            range_end = channels[i];
        }
    }
    
    // Add final range
    if range_start == range_end {
        result.push(format!("{}", range_start));
    } else if range_end == range_start + 1 {
        result.push(format!("{}, {}", range_start, range_end));
    } else {
        result.push(format!("{}-{}", range_start, range_end));
    }
    
    result.join(", ")
}

/// Format a fixture list for display (e.g., "1-5, 7, 9-12")
fn format_fixture_list(fixtures: &[usize]) -> String {
    if fixtures.is_empty() {
        return String::new();
    }
    
    let mut result = Vec::new();
    let mut range_start = fixtures[0];
    let mut range_end = fixtures[0];
    
    for i in 1..fixtures.len() {
        if fixtures[i] == range_end + 1 {
            range_end = fixtures[i];
        } else {
            // End of range, add to result
            if range_start == range_end {
                result.push(format!("{}", range_start));
            } else if range_end == range_start + 1 {
                result.push(format!("{}, {}", range_start, range_end));
            } else {
                result.push(format!("{}-{}", range_start, range_end));
            }
            range_start = fixtures[i];
            range_end = fixtures[i];
        }
    }
    
    // Add final range
    if range_start == range_end {
        result.push(format!("{}", range_start));
    } else if range_end == range_start + 1 {
        result.push(format!("{}, {}", range_start, range_end));
    } else {
        result.push(format!("{}-{}", range_start, range_end));
    }
    
    result.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_single_channel() {
        let cmd = parse_lighting_command("4a33").unwrap();
        assert_eq!(cmd, Command::SetChannelLevel {
            channels: vec![4],
            level: 33,
        });
    }
    
    #[test]
    fn test_parse_selected_level() {
        let cmd = parse_lighting_command("a50").unwrap();
        assert_eq!(cmd, Command::SetSelectedLevel { level: 50 });
    }
    
    #[test]
    fn test_parse_range() {
        let cmd = parse_lighting_command("1thru10a50").unwrap();
        assert_eq!(cmd, Command::SetChannelLevel {
            channels: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            level: 50,
        });
    }
    
    #[test]
    fn test_parse_range_short() {
        let cmd = parse_lighting_command("1t10a50").unwrap();
        assert_eq!(cmd, Command::SetChannelLevel {
            channels: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            level: 50,
        });
    }
    
    #[test]
    fn test_parse_addition() {
        let cmd = parse_lighting_command("1+3+5a75").unwrap();
        assert_eq!(cmd, Command::SetChannelLevel {
            channels: vec![1, 3, 5],
            level: 75,
        });
    }
    
    #[test]
    fn test_parse_complex() {
        let cmd = parse_lighting_command("1-5+7+9-12a100").unwrap();
        assert_eq!(cmd, Command::SetChannelLevel {
            channels: vec![1, 2, 3, 4, 5, 7, 9, 10, 11, 12],
            level: 100,
        });
    }
    
    #[test]
    fn test_parse_selection_only() {
        let cmd = parse_lighting_command("1thru10").unwrap();
        assert_eq!(cmd, Command::SelectChannels {
            channels: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        });
    }
    
    #[test]
    fn test_parse_selection_only_short() {
        let cmd = parse_lighting_command("1t10").unwrap();
        assert_eq!(cmd, Command::SelectChannels {
            channels: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        });
    }
    
    #[test]
    fn test_parse_full_keyword() {
        let cmd = parse_lighting_command("4afull").unwrap();
        assert_eq!(cmd, Command::SetChannelLevel {
            channels: vec![4],
            level: 100,
        });
    }
    
    #[test]
    fn test_format_channel_list() {
        assert_eq!(format_channel_list(&[1, 2, 3, 4, 5]), "1-5");
        assert_eq!(format_channel_list(&[1, 3, 5]), "1, 3, 5");
        assert_eq!(format_channel_list(&[1, 2, 3, 7, 9, 10, 11]), "1-3, 7, 9-11");
    }
    
    #[test]
    fn test_parse_fixture_intensity() {
        let cmd = parse_lighting_command_with_context("4a33", CommandContext::Lighting).unwrap();
        assert_eq!(cmd, Command::SetFixtureIntensity {
            fixtures: vec![4],
            intensity: 0.33,
        });
    }
    
    #[test]
    fn test_parse_fixture_range() {
        let cmd = parse_lighting_command_with_context("1thru5a75", CommandContext::Lighting).unwrap();
        assert_eq!(cmd, Command::SetFixtureIntensity {
            fixtures: vec![1, 2, 3, 4, 5],
            intensity: 0.75,
        });
    }
    
    #[test]
    fn test_parse_fixture_selection() {
        let cmd = parse_lighting_command_with_context("1thru5", CommandContext::Lighting).unwrap();
        assert_eq!(cmd, Command::SelectFixtures {
            fixtures: vec![1, 2, 3, 4, 5],
        });
    }
    
    #[test]
    fn test_context_aware_parsing() {
        // Same input, different context
        let channel_cmd = parse_lighting_command_with_context("4a50", CommandContext::General).unwrap();
        let fixture_cmd = parse_lighting_command_with_context("4a50", CommandContext::Lighting).unwrap();
        
        assert_eq!(channel_cmd, Command::SetChannelLevel {
            channels: vec![4],
            level: 50,
        });
        
        assert_eq!(fixture_cmd, Command::SetFixtureIntensity {
            fixtures: vec![4],
            intensity: 0.5,
        });
    }
    
    #[test]
    fn test_format_fixture_list() {
        assert_eq!(format_fixture_list(&[1, 2, 3, 4, 5]), "1-5");
        assert_eq!(format_fixture_list(&[1, 3, 5]), "1, 3, 5");
        assert_eq!(format_fixture_list(&[1, 2, 3, 7, 9, 10, 11]), "1-3, 7, 9-11");
    }
}
