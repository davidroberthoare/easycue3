# Media Directory Auto-Resolution

**Date:** April 30, 2026  
**Feature:** Automatic fallback to `media/` directory for audio files  
**Status:** Implemented ✅

## Overview

EasyCue3 now automatically resolves audio file paths with a fallback to the `media/` directory. This allows show files to reference audio files by simple filenames (e.g., `sample1.mp3`) without needing full paths.

## How It Works

### Loading Audio Files

When an audio cue is played, the path resolution follows this logic:

1. **Check if path is absolute or exists as-is** → Use it directly
2. **Try prepending `media/` directory** → If `media/sample1.mp3` exists, use it
3. **Fall back to original path** → Use the original path (will fail with proper error)

```rust
// Show file contains: "audio_path": "sample1.mp3"
// Automatically resolved to: "media/sample1.mp3" (if it exists)
```

### Saving Show Files

When saving audio cues, paths are automatically simplified:

- Files in the `media/` directory → Saved as just the filename
- Files elsewhere → Saved with full path

```rust
// User selects: "media/background.mp3"
// Saved as:     "background.mp3"

// User selects: "/home/user/music/song.mp3"
// Saved as:     "/home/user/music/song.mp3"
```

## Example Show File

```json
{
  "audio_cues": [
    {
      "number": 1.0,
      "label": "Background Music",
      "audio_path": "sample1.mp3",  ← Automatically resolved to media/sample1.mp3
      "volume": 0.8,
      "fade_in": 0.0,
      "fade_out": 0.0
    }
  ]
}
```

## Media Directory Structure

```
easycue3/
├── media/
│   ├── sample1.mp3  ← Referenced as "sample1.mp3" in show files
│   ├── sample2.mp3
│   └── sample3.mp3
├── shows/
│   └── example_show.json  ← Contains simple filenames
└── ...
```

## API Reference

### AudioCue Methods

```rust
impl AudioCue {
    /// Get the resolved filesystem path (with media/ fallback)
    pub fn resolved_path(&self) -> PathBuf;
    
    /// Get the canonical path for saving (strips media/ prefix)
    pub fn canonical_path(&self) -> PathBuf;
    
    /// Set path from full path (auto-simplifies if in media/)
    pub fn set_path(&mut self, path: PathBuf);
}
```

### AudioPlayer

```rust
impl AudioPlayer {
    /// Play an audio file (automatically resolves path)
    pub fn play(&mut self, path: &Path, volume: f32) -> Result<()>;
}
```

## Benefits

1. **Cleaner Show Files**: Show files contain simple filenames instead of full paths
2. **Portability**: Shows work across different systems without path rewrites
3. **Organization**: All media files in one standardized location
4. **Backward Compatible**: Absolute paths and existing relative paths still work
5. **Transparent**: Resolution happens automatically, no user action needed

## Implementation Details

### Files Modified

- `src/audio/player.rs` - Added `resolve_audio_path()` helper function
- `src/audio/types.rs` - Added `resolved_path()`, `canonical_path()`, and `set_path()` methods
- `src/ui/sound_cues.rs` - Updated file existence checks to use resolved paths

### Resolution Function

```rust
fn resolve_audio_path(path: &Path) -> PathBuf {
    // If path is absolute or exists as-is, use it
    if path.is_absolute() || path.exists() {
        return path.to_path_buf();
    }
    
    // Try prepending "media/" directory
    let media_path = PathBuf::from("media").join(path);
    if media_path.exists() {
        return media_path;
    }
    
    // Fall back to original path
    path.to_path_buf()
}
```

## Testing

### Manual Test Procedure

1. **Place audio files in media/ directory:**
   ```bash
   ls media/
   # sample1.mp3  sample2.mp3  sample3.mp3
   ```

2. **Load example_show.json:**
   ```bash
   cargo run
   # Open Shows → Load → example_show.json
   ```

3. **Verify audio cues load correctly:**
   - Check Sound Cues panel
   - Files should show green ✓ (file exists)
   - Filenames displayed correctly

4. **Play audio cues:**
   - Click GO in Sound Cues panel
   - Audio should play from media/ directory
   - Log should show: "Playing audio file: media/sample1.mp3"

5. **Save show file:**
   - Modify a cue
   - Save the show
   - Verify JSON still contains simple filenames:
     ```json
     "audio_path": "sample1.mp3"
     ```

### Expected Log Output

```
[INFO] Resolved audio path: sample1.mp3 -> media/sample1.mp3
[INFO] Playing audio file: media/sample1.mp3
```

## Error Handling

### File Not Found

If a file doesn't exist in either location:

```
[ERROR] Failed to play audio cue 1.0: Failed to open audio file: sample1.mp3
```

The error message shows the original path, making it clear which file is missing.

### Invalid Audio Format

If the file exists but can't be decoded:

```
[ERROR] Failed to play audio cue 1.0: Failed to decode audio file
```

## Future Enhancements

- [ ] Add media browser UI for selecting files from media/ directory
- [ ] Support for video files (when video feature is implemented)
- [ ] Media file validation on show load
- [ ] Auto-copy external files to media/ directory when adding to show
- [ ] Media library management (import/export, organize)

## Related Documentation

- [Audio System](src/audio/README.md) - Audio playback architecture
- [Show Files](src/show/mod.rs) - Show file format and persistence
