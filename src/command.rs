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
    /// Clear command line
    Clear,
    /// Invalid/unparseable command
    Invalid(String),
}

/// Parse a lighting command string
/// 
/// Syntax:
/// - Numbers: channel selection
/// - "a" or "@": "at level" operator
/// - "thru" or "-": range operator
/// - "+" or ",": addition operator
/// 
/// Examples:
/// - "4a33" -> Channel 4 at 33%
/// - "a50" -> Set selected channels to 50%
/// - "4" -> Select channel 4
/// - "1thru10" -> Select channels 1-10
pub fn parse_lighting_command(input: &str) -> Result<Command> {
    let input = input.trim().to_lowercase();
    
    if input.is_empty() {
        return Ok(Command::Clear);
    }
    
    // Split on "a" or "@" to separate channel selection from level
    let parts: Vec<&str> = if input.contains('a') {
        input.splitn(2, 'a').collect()
    } else if input.contains('@') {
        input.splitn(2, '@').collect()
    } else {
        // No level specified, just channel selection
        return Ok(Command::SelectChannels {
            channels: parse_channel_selection(input.as_str())?,
        });
    };
    
    if parts.len() != 2 {
        bail!("Invalid syntax: expected 'channels @ level'");
    }
    
    // Check if channel part is empty (e.g., "a50" with no channels)
    if parts[0].trim().is_empty() {
        // Set level on currently selected channels
        let level = parse_level(parts[1])?;
        return Ok(Command::SetSelectedLevel { level });
    }
    
    let channels = parse_channel_selection(parts[0])?;
    let level = parse_level(parts[1])?;
    
    Ok(Command::SetChannelLevel { channels, level })
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

/// Execute a parsed command
pub fn execute_command(cmd: Command, app: &mut crate::app::EasyCueApp) {
    match cmd {
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
}
