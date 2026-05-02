# EasyCue3 Command-Line System

## Overview

EasyCue3 features an ETC EOS-style command-line interface for rapid lighting control. Commands are context-aware and change behavior based on which panel is active.

## Active Pane Tracking

The system tracks which panel is currently "active" (hovered or focused):

- **Lighting Context**: Active when Channels or Lighting Cues panel is focused
- **Sound Context**: Active when Sound Cues panel is focused  
- **General Context**: All other panels

The context indicator appears in the status bar:
- 💡 = Lighting context
- 🔊 = Sound context
- ⌨ = General context

## Command-Line UI

The command line is permanently visible in a single-row footer at the bottom:

```
⏹ Stopped | Q1.0 | 💡 [4+6+8] ⏎ ✖ | Ch 4, 6, 8 | DMX: Virtual
```

**Left:** Playback status + current cue  
**Center:** Command line with context indicator (💡/🔊)  
**Right:** Status message + DMX backend

The command line **automatically rebuilds** based on your channel selection:
- Click channel 4 → shows `4`
- Ctrl+Click channel 6 → shows `4+6`
- Shift+Click channel 10 → shows `4+6+6thru10`

Just type the level (e.g., `a50`) and press Enter!

## Lighting Command Syntax

### Basic Channel Selection

```
4        → Select channel 4
1thru10  → Select channels 1-10
1t10     → Same as above ('t' shorthand for 'thru')
1-10     → Same as above (hyphen works too)
1+3+5    → Select channels 1, 3, and 5
```

### Setting Levels ("At" operator)

Use `a` or `@` to set levels:

```
4a33           → Channel 4 at 33%
1thru10a50     → Channels 1-10 at 50%
1t10a50        → Same as above (using 't' shorthand)
1+3+5a75       → Channels 1, 3, 5 at 75%
1-5+7+9-12a100 → Channels 1-5, 7, 9-12 at 100%
a50            → Set currently selected channels to 50%
```

The last syntax (`a50`) is especially useful: **select channels with clicks, then type a level**.

### Special Level Keywords

```
4afull  → Channel 4 at 100% (full)
4afl    → Same as above
4af     → Same as above
4aout   → Channel 4 at 0% (out)
4ao     → Same as above
```

### DMX Values (0-255)

Values over 100 are interpreted as DMX values (0-255):

```
4a255   → Channel 4 at DMX 255 (100%)
4a127   → Channel 4 at DMX 127 (~50%)
```

## Mouse Interactions in Channels Panel

Channel clicks automatically update the command line by rebuilding it from your selection:

- **Click**: Select single channel → command line shows `4`
  - Replaces previous selection

- **Shift+Click**: Add channel range to selection → command line shows `1thru10`
  - Creates range from last clicked channel to this one

- **Ctrl/Cmd+Click**: Toggle channel in selection → command line shows `1+4+7`
  - Adds or removes channel from selection
  - Command line always reflects current selection

- **Drag**: Adjust channel level (vertical or horizontal drag)
  - Dragging a channel also **selects it** (or updates all selected channels if multiple are selected)
  - Selected channels can then be affected by "a33" type commands

**Key insight:** The command line is always a live representation of your selected channels. It rebuilds automatically with optimal range notation (e.g., `1thru5` instead of `1+2+3+4+5`).

**Important:** Any action that modifies channel levels (typing `1-3a55` or dragging a channel) automatically **selects those channels**. This means you can immediately follow up with "a75" to adjust them further.

## Keyboard Input (Lighting Context)

When the Channels or Lighting Cues panel is active:

- **Numbers (0-9)**: Add to command
- **a or @**: "At" operator for level setting
- **+, ,**: Addition operator for multiple channels
- **-, thru, t**: Range operators (1-10, 1thru10, or 1t10)
- **f, l, o, u**: For keywords like "full", "out"
- **Backspace**: Delete last character
- **Enter**: Execute command
- **Escape**: Clear command (future)

## Examples

### Example 1: Set a Single Channel
```
Workflow:
1. Click/hover on Channels panel (activates lighting context)
2. Type: 4a50
3. Press Enter
Result: Channel 4 set to 50%
```

### Example 2: Set Multiple Channels
```
Workflow:
1. Type: 1thru10a75  (or shorthand: 1t10a75)
2. Press Enter
Result: Channels 1-10 set to 75%
```

### Example 3: Mixed Selection
```
Workflow:
1. Type: 1-5+7+9-12afull
2. Press Enter
Result: Channels 1, 2, 3, 4, 5, 7, 9, 10, 11, 12 set to 100%
```

### Example 4: Select Then Set Level (Recommended!)
```
Workflow:
1. Click channel 4 → command shows "4"
2. Ctrl+Click channel 8 → command shows "4+8"
3. Type: a50 (or just clear and type "a50")
4. Press Enter
Result: Channels 4 and 8 set to 50%

Note: You can type "a50" directly without the channel numbers 
      in the command line - it uses your selection!
```

### Example 5: Building with Ctrl+Click
```
Workflow:
1. Click channel 1 → command shows "1"
2. Ctrl+Click channel 5 → command shows "1+5"
3. Ctrl+Click channel 10 → command shows "1+5+10"
4. Type: a75
5. Press Enter
Result: Channels 1, 5, 10 set to 75%
```

### Example 6: Range with Shift+Click
```
Workflow:
1. Click channel 1 → command shows "1"
2. Shift+Click channel 10 → command shows "1thru10"
3. Type: a100
4. Press Enter
Result: Channels 1-10 set to 100%
```

### Example 7: Drag Then Adjust
```
Workflow:
1. Type: 1thru5a40
2. Press Enter → Channels 1-5 set to 40% and become selected
3. Type: a60 (just the level!)
4. Press Enter → Those same channels 1-5 now at 60%

Alternative:
1. Click-drag channel 7 up to ~30%
2. Channel 7 is now selected
3. Type: a50
4. Press Enter → Channel 7 now at 50%
Result: Level commands automatically select those channels!
```

### Example 8: Complex Selection
```
Workflow:
1. Click channel 1 → "1"
2. Shift+Click channel 5 → "1thru5"
3. Ctrl+Click channel 8 → "1thru5+8"
4. Ctrl+Click channel 10 → "1thru5+8+10"
5. Shift+Click channel 12 → "1thru5+8+10thru12"
6. Type: a60
7. Press Enter
Result: Channels 1-5, 8, 10-12 set to 60%
```

## Command Execution

When a command is executed:

1. **Parse**: Command string is parsed according to syntax rules
2. **Validate**: Channels are validated (1-512), levels validated (0-100)
3. **Execute**: DMX values are written to universe
4. **Feedback**: Status message displays what was done
5. **Clear**: Command line is cleared for next command

## Error Handling

If a command fails to parse, an error message appears in the status bar:

```
Error: Invalid channel: abc
Error: Channel must be between 1 and 512
Error: Range start must be <= end
```

## Future Enhancements

Planned features:

- **Groups**: `g1a50` → Group 1 at 50%
- **Record**: `r` → Record current state as new cue
- **Cue Go**: `q1` or `go` → Jump to cue 1 or advance
- **Effects**: `1thru10@ramp5` → Create chase effect
- **Palettes**: `1thru10p1` → Apply color palette 1
- **Selection recall**: `.` → Recall last selection
- **Escape key**: Clear command line

## Tips for ETC EOS Users

If you're familiar with EOS consoles:

- Basic syntax is very similar: `1 Thru 10 @ 50 Enter`
- Type without spaces (more compact): `1thru10a50`
- Click channels to build commands (replaces typing channel numbers)
- Shift+Click for ranges (like using "Thru" button on console)
- Command line always visible (similar to EOS command display)
- Future: More EOS features coming (groups, effects, palettes)

## Differences from EOS

- Uses `a` instead of `@` by default (both work)
- Case-insensitive: `THRU`, `thru`, `Thru` all work
- Compact syntax: `1-10` works same as `1thru10`
- Click integration for mouse/keyboard hybrid workflow (no Alt needed)
- Command line always visible (permanent in footer)
- Context-aware based on active panel
