# MacAmp Project Guide

> **Note:** For general development guidelines, tooling documentation, and multi-step workflows, see [AGENTS.md](./AGENTS.md)

> **📚 COMPREHENSIVE DOCUMENTATION:** For complete architecture, patterns, and implementation details:
>
> - **Primary Reference:** `docs/MACAMP_ARCHITECTURE_GUIDE.md` (2,728 lines) ⭐
> - **Coding Patterns:** `docs/IMPLEMENTATION_PATTERNS.md` (1,061 lines)
> - **Sprite System:** `docs/SPRITE_SYSTEM_COMPLETE.md` (814 lines)
> - **Skin Format:** `docs/WINAMP_SKIN_VARIATIONS.md` (652 lines)
> - **Full Index:** `docs/README.md` - Complete documentation navigation

## Project Overview

MacAmp is a faithful Winamp recreation for macOS 15+ (Sequoia) and macOS 26+ (Tahoe), leveraging modern SwiftUI enhancements while maintaining classic Winamp aesthetics and behavior.

### Target Platforms

- **Primary:** macOS 15.0+ (Sequoia)
- **Future:** macOS 26.0+ (Tahoe) - Latest SwiftUI features
- **Architecture:** Apple Silicon (arm64) + Intel (x86_64)

## Build Configuration

### Required Build Flags

**Thread Sanitizer (Required):**

```bash
# Always include when building for testing
xcodebuild -enableThreadSanitizer YES [other params]

# MCP tool usage
mcp__XcodeBuildMCP__build_run_macos({
  scheme: "MacAmp",
  extraArgs: ["-enableThreadSanitizer", "YES"]
})
```

### Development Build Commands

```bash
# Standard debug build with sanitizer
xcodebuild -scheme MacAmp -configuration Debug -enableThreadSanitizer YES

# Run tests with sanitizer
xcodebuild test -scheme MacAmp -enableThreadSanitizer YES
```

## Project Architecture

### Directory Structure

```text
MacAmpApp/
├── Audio/           # AVAudioEngine integration, playback logic
├── Models/          # @Observable models (AppSettings, PlaylistManager, etc.)
├── Skins/           # Winamp skin loading, parsing, rendering
├── Utilities/       # Helper functions, extensions
├── ViewModels/      # Legacy ObservableObject VMs (migrating to @Observable)
├── Views/           # SwiftUI views (Main, Playlist, Equalizer, etc.)
└── Windows/         # Custom NSWindow subclasses for Winamp windows
```

### Key Design Patterns

**Architecture:**

- **Three-Layer Pattern:** Mechanism → Bridge → Presentation layers
- See: `docs/MACAMP_ARCHITECTURE_GUIDE.md` §3 for complete layer breakdown

**State Management:**

- **Modern:** `@Observable` macro (Swift 5.9+) for new code
- **Legacy:** `ObservableObject` for existing view models (migrate incrementally)
- **Persistence:** `UserDefaults` with `didSet` handlers in `@Observable` classes
- See: `docs/IMPLEMENTATION_PATTERNS.md` §2 for state patterns

**Audio Pipeline:**

- **Dual Backend:** AVAudioEngine (local files) + AVPlayer (internet radio streams)
- **Orchestration:** PlaybackCoordinator manages backend switching
- **Formats:** MP3, FLAC, WAV, M4A, OGG (local), HTTP/HTTPS streams (radio)
- **Visualization:** 19-bar Goertzel-like spectrum analyzer via AVAudioEngine tap
- **EQ:** Only available for local files (AVAudioEngine limitation)
- See: `docs/MACAMP_ARCHITECTURE_GUIDE.md` §4 for dual backend architecture

**UI Architecture:**

- **SwiftUI-first:** All new UI components in SwiftUI
- **AppKit bridging:** `NSWindow` subclasses for custom window behavior
- **Material effects:** `.material(.hudWindow)` for authentic Winamp look
- **Semantic Sprites:** Use `SpriteResolver` with semantic IDs (never hard-code sprite names)
- See: `docs/SPRITE_SYSTEM_COMPLETE.md` for complete sprite resolution system

## Winamp Compatibility Requirements

### Skin System

**Format Support:**

- Classic Winamp skins (WSZ/ZIP format)
- `main.bmp`, `playpaus.bmp`, `cbuttons.bmp`, etc.
- `region.txt` for window shapes
- `pledit.txt` for playlist styling

**Rendering:**

- 1:1 pixel accuracy at native resolution
- Double-size mode (2x scaling)
- Proper button state transitions (normal, pressed, active)

**Conventions:**

```swift
// Skin resource paths always relative to skin bundle
let mainBitmap = skinURL.appendingPathComponent("main.bmp")

// Button regions defined in region.txt
struct ButtonRegion {
    let normal: NSRect
    let pressed: NSRect
    let activeNormal: NSRect?  // Optional for toggle buttons
}
```

### Classic Winamp Behavior

**Window Management:**

- Magnetic docking (snap to screen edges and other MacAmp windows)
- "Always on Top" mode
- Custom shaped windows via `region.txt`
- Window clustering (Main + Equalizer + Playlist)

**Keyboard Shortcuts:**

- Follow Winamp 5 Modern skin shortcuts where possible
- Ctrl+O: Options menu
- Ctrl+T: Toggle time display
- Ctrl+I: Track info
- Ctrl+R: Cycle repeat modes

**Time Display:**

- Elapsed time (default): `2:34`
- Remaining time: `-1:26`
- Toggle via Ctrl+T or context menu

## Code Style & Conventions

### Swift Coding Standards

**See [AGENTS.md](./AGENTS.md) for comprehensive Swift guidelines**

**MacAmp-Specific:**

```swift
// ALWAYS use @MainActor for UI-related classes
@MainActor
@Observable
final class AppSettings {
    // UserDefaults persistence pattern
    var timeDisplayMode: TimeDisplayMode = .elapsed {
        didSet { UserDefaults.standard.set(timeDisplayMode.rawValue, forKey: "timeDisplayMode") }
    }
}

// Weak references for delegates/targets
weak var menuTarget: AnyObject?

// Explicit access control
private func internalHelper() { }
public func publicAPI() { }
```

### Naming Conventions

**Files:**

- Views: `WinampMainWindow.swift`, `TrackInfoView.swift`
- Models: `AppSettings.swift`, `PlaylistManager.swift`
- Utilities: `SkinLoader.swift`, `AudioEngine.swift`

**Classes:**

- SwiftUI Views: `TrackInfoView`, `UnifiedDockView`
- NSWindow subclasses: `ShapedWindow`, `VideoWindow`
- Managers: `AudioEngineManager`, `SkinManager`

**Properties:**

- Settings: `timeDisplayMode`, `isShuffleEnabled`
- UI State: `isOptionsMenuVisible`, `currentTrackIndex`
- Resources: `mainWindowSkin`, `playlistFont`

## Testing Strategy

### Required Testing

**Before Committing:**

```bash
# 1. Build with Thread Sanitizer
xcodebuild -scheme MacAmp -configuration Debug -enableThreadSanitizer YES

# 2. Run test suite
xcodebuild test -scheme MacAmp -enableThreadSanitizer YES

# 3. Verify no sanitizer warnings
# Check console for data race warnings, threading issues
```

**Integration Testing:**

- Load 3-5 different Winamp skins (verify rendering)
- Test magnetic window docking
- Verify audio playback (local files + internet radio)
- Check keyboard shortcuts
- Test double-size mode

### Common Issues to Check

- **Data races:** Always use `@MainActor` for UI classes
- **Retain cycles:** Use `weak` for delegates, `[weak self]` in closures
- **Menu lifecycle:** NSMenu must be retained (use instance variables)
- **Window hierarchy:** Proper parent-child relationships for magnetic docking

## Feature Development Workflow

### 1. Research Phase

```bash
# Use Gemini for historical research
gemini -p "Research how Winamp 2.x/5.x implemented [feature].
Include UI patterns, behavior, and user expectations."

# Check existing MacAmp patterns (Swift)
rg "similar_pattern" --type swift
sg --lang swift -p 'pattern $NAME { $$$ }'

# Search reference repositories (TypeScript/JavaScript)
rg "pattern_name" --type ts --type js
sg --lang typescript -p 'interface $NAME { $$$ }'
sg --lang javascript -p 'function $NAME($$$) { $$$ }'
```

### 2. Implementation Phase

**Reference Tasks:**

- Check `tasks/done/` for similar completed features
- Review patterns in `tasks/[active-task]/research.md`
- Document decisions in `tasks/[task-name]/plan.md`

**Code Review:**

```bash
# Use Codex Oracle for code review before committing
codex "@File1.swift @File2.swift Review this implementation for:
- Thread safety (@MainActor usage)
- Memory management (weak references)
- Winamp compatibility
- Integration with existing patterns"
```

### 3. Verification Phase

- Build + run with Thread Sanitizer
- Test with multiple skins
- Verify against Winamp 5 Modern behavior
- Check performance (no frame drops in visualization)

## Common Patterns

### AppKit-SwiftUI Bridging

```swift
// Hosting SwiftUI in NSWindow
let hostingController = NSHostingController(rootView: swiftUIView)
window.contentViewController = hostingController

// NSMenu from SwiftUI context
class MenuItemTarget {
    @objc func menuAction(_ sender: NSMenuItem) {
        // Trampoline to Swift closure
    }
}
```

### Audio Visualization

```swift
// AVAudioEngine tap for visualization data
let tap = AVAudioMixerNode()
tap.installTap(onBus: 0, bufferSize: 1024, format: format) { buffer, time in
    // Process audio samples for waveform/spectrum
}
```

### Skin Resource Loading

```swift
// Load bitmap from skin bundle
func loadBitmap(_ name: String) -> NSImage? {
    guard let url = skinURL?.appendingPathComponent(name) else { return nil }
    return NSImage(contentsOf: url)
}

// Parse region.txt for button coordinates
func parseRegions() -> [String: ButtonRegion] {
    // Parse Winamp region.txt format
}
```

### Semantic Sprite Resolution

**DO NOT hard-code sprite filenames.** Use semantic IDs:

```swift
// ❌ WRONG: Hard-coded sprite names
let playImage = loadBitmap("play.bmp")

// ✅ CORRECT: Semantic sprite resolution
let playImage = spriteResolver.resolve(.playButton, state: .normal)
```

The SpriteResolver handles:

- Skin variations (different file names/layouts)
- Fallback generation for missing sprites
- State transitions (normal/pressed/active)

See: `docs/SPRITE_SYSTEM_COMPLETE.md` for complete sprite system architecture

## Oracle (Codex) Usage

### When to Consult Oracle

**Before implementing complex features:**

```bash
codex "@relevant/files.swift Plan review:
- Is this approach thread-safe?
- Does it follow MacAmp patterns?
- Are there Winamp compatibility concerns?"
```

**After implementation:**

```bash
codex "@implemented/files.swift Code review:
- Check for retain cycles
- Verify @MainActor annotations
- Confirm UserDefaults persistence pattern
- Validate against Winamp behavior"
```

**For architecture decisions:**

```bash
codex "@Audio/ @Models/ Should this be @Observable or ObservableObject?
Analyze impact on existing code and migration path."
```

## References

### MacAmp Documentation

- **Architecture Guide:** `docs/MACAMP_ARCHITECTURE_GUIDE.md` (2,728 lines) ⭐ PRIMARY REFERENCE
- **Coding Patterns:** `docs/IMPLEMENTATION_PATTERNS.md` (1,061 lines)
- **Sprite System:** `docs/SPRITE_SYSTEM_COMPLETE.md` (814 lines)
- **Skin Format:** `docs/WINAMP_SKIN_VARIATIONS.md` (652 lines)
- **Build & Release:** `docs/RELEASE_BUILD_GUIDE.md` (447 lines)
- **Full Index:** `docs/README.md` - Complete documentation navigation

### General Guidelines

- **Development Workflows:** [AGENTS.md](./AGENTS.md) - Tooling, workflows, Swift standards
- **Task Documentation:** `tasks/` - Historical implementations and patterns
- **Xcode 26 Docs:** See AGENTS.md for Tahoe SwiftUI enhancements

## Quick Links

```bash
# Build and run with sanitizer
mcp__XcodeBuildMCP__build_run_macos({ scheme: "MacAmp", extraArgs: ["-enableThreadSanitizer", "YES"] })

# Search for similar implementations (Swift)
rg "pattern_name" --type swift
sg --lang swift -p 'func $NAME($$$) { $$$ }'

# Search reference repos (TypeScript/JavaScript)
rg "pattern_name" --type ts --type js
sg --lang typescript -p 'const $NAME = ($$$) => { $$$ }'
sg --lang javascript -p 'async function $NAME($$$) { $$$ }'

# Research Winamp behavior
gemini -p "How did Winamp [version] implement [feature]?"

# Code review with Oracle
codex "@files.swift Review for thread safety and Winamp compatibility"
```
