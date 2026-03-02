# Calliope — Design Document

**Version:** 0.1 (Planning)
**Status:** Draft

---

## Design Philosophy

Calliope is a background utility. It should feel like a trusted, quiet companion — present when you need it, invisible when you don't. The visual language is **soft and approachable**: warm without being childish, friendly without being loud. Every UI surface should feel like it belongs on the desktop alongside tools like Things 3 or Bear — considered, calm, and human.

The design should never shout. Recording state is communicated through gentle motion, not alarm-red warnings. Settings should feel like a thoughtful preferences pane, not a configuration dashboard.

---

## Color System

### Approach: System-adaptive with manual override

Calliope follows the OS-level light/dark preference by default. Users can override this in Settings → Appearance. There are two themes: **Light** and **Dark**. No sepia, no custom themes in v1.

### Palette

All colors are defined as CSS custom properties and mapped to semantic tokens. There is **no accent color** — the palette is monochromatic, relying entirely on contrast, weight, and spacing to create hierarchy.

#### Light Theme

| Token | Value | Usage |
|-------|-------|-------|
| `--bg-base` | `#FAFAF9` | App background, window base |
| `--bg-surface` | `#FFFFFF` | Cards, popovers, input fields |
| `--bg-subtle` | `#F5F4F2` | Sidebar backgrounds, hover states, dividers |
| `--bg-muted` | `#EDECEA` | Disabled surfaces, secondary areas |
| `--text-primary` | `#1C1B1A` | Headings, body text, labels |
| `--text-secondary` | `#6B6966` | Descriptions, captions, placeholder text |
| `--text-disabled` | `#B0ADA9` | Disabled labels |
| `--border` | `#E4E2DF` | Subtle dividers, input outlines |
| `--border-strong` | `#C9C6C2` | Focused inputs, active sections |
| `--recording` | `#1C1B1A` | Waveform and active recording indicators |
| `--error` | `#C0392B` | Error states only |

#### Dark Theme

| Token | Value | Usage |
|-------|-------|-------|
| `--bg-base` | `#141413` | App background |
| `--bg-surface` | `#1E1D1C` | Cards, popovers, inputs |
| `--bg-subtle` | `#252422` | Hover states, dividers |
| `--bg-muted` | `#2C2B29` | Disabled surfaces |
| `--text-primary` | `#F0EFED` | Headings, body text |
| `--text-secondary` | `#908D89` | Captions, descriptions |
| `--text-disabled` | `#5A5754` | Disabled labels |
| `--border` | `#2E2D2B` | Dividers |
| `--border-strong` | `#48453F` | Focused inputs |
| `--recording` | `#F0EFED` | Waveform and recording indicators |
| `--error` | `#E74C3C` | Error states only |

> The warm gray base (slightly yellowish-gray, not blue-gray) is intentional — it gives the app a softer, more organic feel compared to the cool neutrals common in developer tools.

---

## Typography

### Typeface: Geist

Geist is bundled with the app as a WOFF2 subset. It is used for all UI text. Monospace variant (Geist Mono) is used for hotkey labels and model names.

### Scale

| Name | Size | Weight | Line Height | Usage |
|------|------|--------|-------------|-------|
| `title` | 15px | 600 | 1.3 | Window titles, section headings |
| `body` | 13px | 400 | 1.5 | Body text, descriptions, labels |
| `body-medium` | 13px | 500 | 1.5 | Button labels, active nav items |
| `caption` | 11px | 400 | 1.4 | Helper text, timestamps, small labels |
| `mono` | 12px | 400 | 1.4 | Hotkey badges, model file names (Geist Mono) |

All font sizes are in `px` (not `rem`) since this is a desktop app with fixed-density UI — no browser zoom scaling needed.

---

## UI Surfaces

Calliope has two primary UI surfaces: the **Tray Popover** and the **Settings Window**.

---

### Surface 1: Tray Popover

The popover appears when the user clicks the system tray icon. It is the primary day-to-day interface.

**Dimensions:** 280px wide × variable height (max ~420px)
**Position:** Anchored to tray icon, above on Windows/Linux, below on macOS (following OS convention)
**Border radius:** 12px
**Shadow:** Soft drop shadow (no colored glow)

#### Layout

```
╭────────────────────────────────╮
│                                │
│  ◉  Idle                       │  ← Status row: icon + state label
│                                │
│ ─────────────────────────────  │  ← Hairline divider
│                                │
│  Model                         │  ← Section label (caption)
│  ┌──────────────────────────┐  │
│  │  large-v3-turbo       ▾  │  │  ← Dropdown selector
│  └──────────────────────────┘  │
│                                │
│  Mode                          │
│  ┌────────────┐ ┌────────────┐ │
│  │ Push-to-talk│ │   Toggle  │ │  ← Segmented control
│  └────────────┘ └────────────┘ │
│                                │
│  Hotkey                        │
│  ┌──────────────────────────┐  │
│  │  ⌥ Space              ✎  │  │  ← Hotkey display + edit button
│  └──────────────────────────┘  │
│                                │
│ ─────────────────────────────  │
│                                │
│  Open Settings          Quit   │  ← Footer actions
│                                │
╰────────────────────────────────╯
```

#### State Variations

The **status row** changes based on app state:

| State | Icon | Label | Behavior |
|-------|------|-------|----------|
| Idle | `◉` (small filled circle, muted) | "Ready" | Static |
| Recording | Animated waveform (see below) | "Listening..." | Waveform animates to mic input |
| Transcribing | Subtle spinner or pulsing dots | "Transcribing..." | Brief (usually <2s) |
| Error | `!` warning mark | Short error message | Tappable to see detail |

When **recording**, the status row expands to show the live waveform visualizer (see Recording Feedback section). The rest of the popover dims slightly to draw attention to the active state.

---

### Surface 2: Settings Window

**Dimensions:** 580px wide × 480px tall (fixed, non-resizable)
**Border radius:** 12px (macOS sheet style), or OS window chrome on Windows/Linux
**Layout:** Horizontal tabs at the top, content pane below

#### Tab Bar

```
╭──────────────────────────────────────────────────────────╮
│  General   Models   Hotkeys   Post-processing   Appearance│
│ ─────────────────────────────────────────────────────────│
│                                                           │
│  [tab content]                                            │
│                                                           │
╰──────────────────────────────────────────────────────────╯
```

Tabs use `body-medium` weight. Active tab has a 1.5px underline in `--text-primary`. Inactive tabs are `--text-secondary`. No background fills on tabs.

#### Tab: General

- Recording mode (Push-to-talk / Toggle) — segmented control
- Audio input device — dropdown with detected microphones
- Input language — searchable dropdown (auto-detect default)
- Launch at login — toggle
- Injection fallback behavior — toggle ("Copy to clipboard if injection fails")

#### Tab: Models

Full-width model browser. Each model shown as a row:

```
┌────────────────────────────────────────────────────────┐
│ large-v3-turbo     809 MB   ████████░░  Fast / High   ▼│
│ ✓ Downloaded                            [ Set Active ] │
└────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────┐
│ large-v3           1.5 GB   ████████████ Slow / Best  ▼│
│ Not downloaded                              [ Download ]│
└────────────────────────────────────────────────────────┘
```

- Speed and quality shown as a short text label + subtle bar
- Active model has a checkmark and highlighted border
- Download shows inline progress bar, cancellable
- Delete option revealed on hover/expand

#### Tab: Hotkeys

- Push-to-talk hotkey recorder — click to capture, press keys, confirm
- Toggle hotkey recorder — same
- Conflict warnings shown inline (e.g. "This shortcut is used by Spotlight")

#### Tab: Post-processing

Two collapsible sections, both collapsed by default:

**Local (Ollama)**
- Enable toggle
- Ollama endpoint (default: `http://localhost:11434`)
- Model name (text input, e.g. `llama3.2:3b`)
- System prompt (textarea, editable)
- Connection test button

**Cloud**
- Enable toggle
- Provider selector (OpenAI / Anthropic)
- API key input (masked, stored in OS keychain)
- Model (e.g. `gpt-4o-mini`)
- System prompt (textarea, editable)
- Privacy notice: "Audio is never sent to the cloud. Only the transcribed text is processed."

#### Tab: Appearance

- Theme — segmented control: System / Light / Dark

---

## Recording Feedback

### Waveform Visualizer

When recording is active, a waveform visualization is displayed in the tray popover's status area. It replaces the static status icon.

**Style:**
- A horizontal row of ~28 bars
- Each bar is a thin rounded rectangle (2px wide, 2px gap, 2–24px tall)
- Bar heights animate to real-time audio input amplitude using FFT or simple RMS energy bands
- Bars are colored `--recording` (near-black in light mode, near-white in dark mode)
- The waveform has a subtle fade-out at both left and right edges (via CSS mask gradient)
- Animation is smooth (60fps using `requestAnimationFrame` or CSS transitions on bar heights)

**Idle state fallback:** When not recording, the bars show a flat, centered line at minimum height (3px). This makes the transition into recording state feel natural rather than jarring.

**Silence detection:** Bars drop to minimum height when VAD detects silence, giving visual feedback that no speech is being captured.

---

## Component Library

All custom components are built from scratch (no third-party component library). This keeps the bundle lean and ensures pixel-level control over the warm, native feel.

### Core Components

| Component | Description |
|-----------|-------------|
| `Button` | Primary (filled), Secondary (outlined), Ghost variants. Rounded corners (6px). No icon-only variant in v1. |
| `SegmentedControl` | 2–3 options, full-width or auto-width. Active segment: bg-surface with border. |
| `Select` | Custom dropdown with keyboard navigation. Native `<select>` used as accessibility fallback. |
| `Toggle` | Pill-style on/off switch. Animated slide. |
| `HotkeyRecorder` | Captures key combos, displays as badge chips (e.g. `⌥` + `Space`). |
| `ProgressBar` | Thin (4px) horizontal bar. Used for model download progress. |
| `Waveform` | Canvas-based real-time audio visualizer (see above). |
| `Tab` / `TabBar` | Underline-style horizontal tabs. |
| `Divider` | 1px horizontal rule in `--border`. |
| `Badge` | Mono-font label for keyboard shortcuts and model names. Subtle `--bg-subtle` fill. |
| `StatusDot` | Small animated circle indicating app state. |

---

## Motion & Animation

Calliope uses animation sparingly. Motion should feel natural, not decorative.

| Element | Animation | Duration | Easing |
|---------|-----------|----------|--------|
| Popover open/close | Fade + 4px vertical translate | 150ms | `ease-out` |
| Tab content switch | Fade | 100ms | `ease` |
| Waveform bars | Height transition | 80ms | `ease-out` |
| Toggle switch | Slide + background color | 150ms | `ease-in-out` |
| Download progress bar | Width transition | continuous | linear |
| State icon transitions | Crossfade | 200ms | `ease` |

No bounce, no spring physics, no overshooting animations. Motion serves communication, not delight.

All animations respect the OS `prefers-reduced-motion` media query — when enabled, all transitions are instant (duration: 0ms).

---

## Iconography

Icons are from the **Lucide** icon set (MIT license), rendered as inline SVGs at 16×16px. Stroke width: 1.5px. Color inherits from text token.

Icons used:
- `mic` — idle/ready state
- `mic-off` — muted / paused
- `loader-2` — transcribing (spinning)
- `alert-triangle` — error state
- `settings` — settings link
- `download` — model download
- `trash-2` — delete model
- `check` — active model indicator
- `edit-2` — edit hotkey
- `x` — cancel / close

Tray icon is a custom monochrome SVG (two variants: idle and recording). It follows OS conventions:
- macOS: template image (black with alpha, OS inverts for dark menubar)
- Windows: 16×16 ICO with light and dark variants
- Linux: follows XDG icon spec; provided at 16, 22, 32px

---

## Onboarding Flow

The first-launch onboarding uses a **stepped modal** (not a separate window). It overlays the popover or opens as a centered floating panel.

**Steps:**

```
Step 1 of 4                        Step 2 of 4
╭──────────────────────────╮       ╭──────────────────────────╮
│                           │       │                           │
│   Welcome to Calliope     │       │   Allow Microphone Access │
│                           │       │                           │
│   Speak naturally.        │       │   Calliope needs access   │
│   Your words appear       │       │   to your microphone to   │
│   wherever you type.      │       │   transcribe your voice.  │
│                           │       │                           │
│   Everything stays on     │       │   [ Grant Access ]        │
│   your device.            │       │                           │
│                           │       │   ● Granted               │
│   [ Get Started ]         │       │   [ Continue ]            │
╰──────────────────────────╯       ╰──────────────────────────╯

Step 3 of 4                        Step 4 of 4
╭──────────────────────────╮       ╭──────────────────────────╮
│                           │       │                           │
│   Download a Model        │       │   You're ready.           │
│                           │       │                           │
│   ● large-v3-turbo        │       │   Hold  ⌥ Space  to start │
│     809 MB  Recommended   │       │   speaking. Release to    │
│     ████████░░░░ 67%      │       │   transcribe.             │
│                           │       │                           │
│   ○ small  (244 MB)       │       │   Open any text field     │
│   ○ base   (74 MB)        │       │   and try it now.         │
│                           │       │                           │
│   Downloading...          │       │   [ Done ]                │
╰──────────────────────────╯       ╰──────────────────────────╯
```

Step indicators shown as small dots at the top of each card. No back button — onboarding is linear. Skip links available on permission steps (for users who want to configure manually).

---

## Accessibility

- All interactive elements are keyboard-navigable (tab order follows visual layout)
- Focus rings use `--border-strong` color, 2px offset
- All icons have `aria-label` or are accompanied by visible text
- Color is never the sole means of conveying information (state is always also communicated via text or icon shape)
- Minimum touch/click target size: 32×32px
- Contrast ratios meet WCAG AA for all text/background pairings
