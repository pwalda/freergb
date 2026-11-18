---
name: freergb
description: A matte lab bench for the room's light; numbered glass vials of looks over an LED dot-matrix stage, dark first.
colors:
  lamp: "#efe6cf"
  lamp-ink: "#121110"
  ground: "#121110"
  rail: "#161513"
  raise: "#1e1d1b"
  stage: "#0b0b0a"
  line: "#2a2825"
  line2: "#383531"
  text: "#e9e5dc"
  muted: "#958f83"
  dot-off: "#22201d"
  dot-line: "#54504a"
  led-edge: "rgba(255, 255, 255, 0.22)"
  led-off: "rgba(255, 255, 255, 0.04)"
  ok: "#9cc7a4"
  warn: "#d9b26a"
  bad: "#e09a8c"
  light-lamp: "#1b1a17"
  light-lamp-ink: "#f8f6f1"
  light-ground: "#f1efea"
  light-rail: "#eae7e0"
  light-raise: "#fbfaf7"
  light-stage: "#e6e2d8"
  light-line: "#dcd8cf"
  light-line2: "#c9c4b9"
  light-text: "#1b1a17"
  light-muted: "#6a655a"
  light-dot-off: "#d8d3c8"
  light-dot-line: "#aca598"
  light-led-edge: "rgba(30, 25, 15, 0.22)"
  light-led-off: "rgba(30, 25, 15, 0.05)"
  light-ok: "#3d7a4d"
  light-warn: "#8a6417"
  light-bad: "#a8442f"
typography:
  display:
    fontFamily: "'Geist Variable', system-ui, -apple-system, 'Segoe UI Variable Text', 'Segoe UI', Roboto, sans-serif"
    fontSize: "34px"
    fontWeight: 600
    lineHeight: 1.05
    letterSpacing: "-0.035em"
  headline:
    fontFamily: "'Geist Variable', system-ui, -apple-system, 'Segoe UI Variable Text', 'Segoe UI', Roboto, sans-serif"
    fontSize: "20px"
    fontWeight: 600
    lineHeight: 1.45
    letterSpacing: "-0.012em"
  title:
    fontFamily: "'Geist Variable', system-ui, -apple-system, 'Segoe UI Variable Text', 'Segoe UI', Roboto, sans-serif"
    fontSize: "15px"
    fontWeight: 600
    lineHeight: 1.45
  body:
    fontFamily: "'Geist Variable', system-ui, -apple-system, 'Segoe UI Variable Text', 'Segoe UI', Roboto, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.45
  control:
    fontFamily: "'Geist Variable', system-ui, -apple-system, 'Segoe UI Variable Text', 'Segoe UI', Roboto, sans-serif"
    fontSize: "13px"
    fontWeight: 500
    lineHeight: 1.45
  label:
    fontFamily: "'Geist Variable', system-ui, -apple-system, 'Segoe UI Variable Text', 'Segoe UI', Roboto, sans-serif"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: 1.45
  readout:
    fontFamily: "'Geist Mono Variable', ui-monospace, 'Cascadia Mono', Consolas, monospace"
    fontSize: "11px"
    fontWeight: 400
    lineHeight: 1.45
    fontFeature: "tnum"
rounded:
  tag: "4px"
  chip: "7px"
  field: "8px"
  button: "9px"
  plate: "10px"
  toast: "11px"
  well: "12px"
  sheet: "14px"
  round: "50%"
spacing:
  hair: "2px"
  xs: "6px"
  sm: "8px"
  md: "10px"
  lg: "14px"
  field-gap: "18px"
  section: "20px"
  main-x: "28px"
  column: "36px"
components:
  button:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    typography: "{typography.control}"
    rounded: "{rounded.button}"
    padding: "0 14px"
    height: "34px"
  button-hover:
    backgroundColor: "{colors.raise}"
  button-sm:
    rounded: "{rounded.button}"
    padding: "0 10px"
    height: "28px"
  icon-button:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.field}"
    size: "30px"
  chip:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    rounded: "{rounded.chip}"
    padding: "0 10px"
    height: "28px"
  chip-selected:
    backgroundColor: "{colors.lamp}"
    textColor: "{colors.lamp-ink}"
  select:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    rounded: "{rounded.field}"
    padding: "0 32px 0 10px"
    height: "32px"
  tag:
    backgroundColor: "{colors.lamp}"
    textColor: "{colors.lamp-ink}"
    rounded: "{rounded.tag}"
    padding: "0 5px"
  tag-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.tag}"
    padding: "0 5px"
  rail-target:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.plate}"
    padding: "10px"
  rail-target-selected:
    backgroundColor: "{colors.raise}"
    textColor: "{colors.text}"
  toast:
    backgroundColor: "{colors.raise}"
    textColor: "{colors.text}"
    rounded: "{rounded.toast}"
    padding: "6px 6px 6px 14px"
  sheet:
    backgroundColor: "{colors.raise}"
    textColor: "{colors.text}"
    rounded: "{rounded.sheet}"
    width: "min(920px, calc(100vw - 32px))"
---

# Design System: freergb

## Overview

**Creative North Star: "The Lab Bench"**

The window is a matte, warm-black bench. Each look is a numbered glass vial of light, and the stage is an LED dot matrix where the room's own hardware lights up. The interface is quiet: hairlines, one raised surface step, bone text, stone secondary text and a single warm lamp white. Everything saturated on screen is light the devices make (on the stage, in the vials, in the rail's mini strips, in the logo's lit dots). The chrome frames that light and never competes with it.

Density is calm and deliberate: one display-size heading for what is playing, the stage as the brightest and largest element, a shelf of six vials as the primary control, and the fine settings below (or in a right column on wide windows) with deeper ones folded into a "More settings" drawer. The power user's depth (built-in modes, painting by hand, the desk arrangement, zone sizes) lives one step down, never hidden.

Rejected for this world: the gamer RGB-suite look (neon chrome, coloured panels, aggressive angles), frosted or translucent glass panels and coloured backdrops, a dashboard of equal device cards, and minimalism so sparse that controls disappear.

**Key Characteristics:**
- Warm near-black ground (light theme: warm paper with no dark areas), separated by 1px hairlines.
- One accent, the lamp: selection, slider fill, focus ring, the "Playing" tag, the toast drain line.
- One LED dot shared by the logo, the stage and the rail's mini strips.
- Geist for the interface, Geist Mono for numbers that are counts or readings.
- Motion that settles (strong ease-out), with a signature look-change wipe and a brimming vial.

## Colors

A muted, matte, warm-neutral palette with one off-white accent; hue belongs to the hardware light alone.

All colours are CSS custom properties on `:root` in `ui/src/app.css`; the light theme reassigns the same names under `:root[data-theme='light']`. Components use only these variables. The frontmatter's `light-*` keys are the light-theme values of the same variables, not separate tokens.

### Primary
- **Lamp** (`--lamp`): the only accent. Selected chip fill, pressed tag ("Playing", "Own look"), slider fill, switch-on track, the focus ring, the own-look dash in the rail, the toast drain line, the painter's selected swatch ring. In the light theme the lamp becomes ink (near-black), so the accent is always "the strongest neutral", never a hue.
- **Lamp Ink** (`--lamp-ink`): text and knobs sitting on a lamp fill.

### Neutral
- **Ground** (`--ground`): the main column and the page behind everything.
- **Rail** (`--rail`): the left rail, one step off the ground.
- **Raise** (`--raise`): the single raised step. Selected rail plate, button and chip hover, toast, painter sheet, select options.
- **Stage Well** (`--stage`): recessed wells where LEDs are edited (the painter's key grid and LED rows).
- **Line** (`--line`): structural hairlines (rail edge, controls divider, sheet header and footer rules).
- **Line 2** (`--line2`): control strokes (button, chip, select, icon button borders), slider and switch tracks, unlit logo dots, scrollbar thumb.
- **Bone Text** (`--text`): primary text, slider and switch knobs.
- **Stone Muted** (`--muted`): secondary text, sublines, value readouts in labels, stage captions and device names, icon colour at rest.
- **Unlit Dot** (`--dot-off`) and **Device Outline** (`--dot-line`): the stage's unlit matrix and the dotted outline of each device template; also the unlit state of mini strips.
- **LED Edge** (`--led-edge`) and **LED Off** (`--led-off`): the rim of a painted LED button and the fill of an unlit one.

### Status
- **Connected** (`--ok`), **Starting / Reconnecting** (`--warn`, pulsing), **Failed** (`--bad`): only as the 6px status dot in the rail, the same dot marking warnings (`--warn`) and errors (`--bad`) in the diagnostics log, and the inline error line (text plus a 45% `--bad` hairline). Deliberately desaturated so they read as state, not decoration.

### Named Rules
**The Only Light Rule.** Saturated colour on screen comes from the hardware's light (stage dots, vial liquid, mini strips, the logo's lit dots) and nowhere else. Chrome, panels and backgrounds stay warm neutral in both themes.

**The One Lamp Rule.** Every "this one" state uses the lamp and nothing else: selection, on, progress, focus. Never introduce a second accent; if something needs emphasis, use weight or the raise step.

**The Paper Rule.** The light theme is warm paper end to end. It has no dark areas: the stage becomes light dots on paper, and the lamp turns to ink.

## Typography

**Display Font:** Geist Variable (with system-ui, Segoe UI Variable, Roboto fallbacks)
**Body Font:** Geist Variable
**Label/Mono Font:** Geist Mono Variable (with ui-monospace, Cascadia Mono, Consolas)
**Wordmark Font:** Doto Variable, dot-matrix, used only for the "freeRGB" wordmark beside the logo: 21px, "free" at 700 in `--muted`, "RGB" at 900 in `--text`, like dots dim and fully lit. Latin subset only.

**Character:** A neutral, precise grotesk with a matching mono, like labels and readouts on lab equipment. Fonts are self-hosted from `@fontsource-variable` (latin and latin-ext subsets) via `@font-face` in `ui/src/app.css`; the window's CSP allows no remote fonts and no `data:` URLs, so new fonts must be bundled the same way.

### Hierarchy
- **Display** (600, 34px; 40px from 1240px, 28px at 700px and below; line-height 1.05, -0.035em, balanced wrap): the name of what is playing on the current target ("Rainbow", a device name, "Effects are off"). One per view.
- **Headline** (600, 20px, -0.012em): sheet titles (the painter).
- **Title** (600, 15px): sub-sections inside a sheet (a zone name in the painter). The rail brand wordmark is its own case (650, 16px, -0.025em).
- **Body** (400, 14px, 1.45): base text, notices on the stage.
- **Control** (500 for buttons and names, 400 for labels; 13 to 13.5px): buttons (13.5px), rail target names (13.5px), field labels, chips (12.5px), the display subline (13.5px muted).
- **Label** (500, 12px, muted, sentence case): group headings ("Targets", "Built-in modes of this device", "Drawn as"), rail target sublines, vial descriptions.
- **Readout** (Geist Mono, tabular figures): LED counts, slider values, vial numbers, the stage caption and device names drawn on the stage (11px). In running text it is applied with the `.num` helper at 0.92em.

### Named Rules
**The Readout Rule.** A number that is a count or a measurement (LEDs, %, ×, vial index) is set in Geist Mono with tabular figures; words never are, except the stage's device names, which are drawn as readouts.

**The Sentence Case Rule.** No uppercase, no tracking-out, no small-caps labels. Hierarchy comes from size, weight and the muted colour.

## Layout

A two-area grid in `ui/src/App.svelte`: a fixed **rail** on the left and a scrolling **main** column.

- **Default (701 to 1239px):** rail 236px. Main is a single column with areas head, stage, vials, controls; padding 24px 28px 90px (bottom room for the toast); 20px gap. Controls sit in a three-column grid (18px row gap, 28px column gap) under a `--line` hairline.
- **Wide (1240px and up):** rail 272px. Main becomes two columns: stage and vials on the left, controls in a 280px right column with a left hairline instead of a top one; 36px column gap; padding 30px 36px; display grows to 40px.
- **Narrow (700px and below):** the rail turns into a top bar (logo, short status, theme switch) with a horizontally scrolling, snap-aligned row of 150px target cards; main padding 18px 16px; controls collapse to one column; vials wrap to two rows of three.
- `--rail-w` is published on `:root` (236px, 272px, 0px) so fixed overlays (the toast) centre over the main column, not the window.
- An error line, when present, takes its own row above the heading.

**Stage sizing:** `clamp(210px, 35vw, 480px)` tall (`clamp(180px, 52vw, 280px)` narrow). The canvas fills it minus a 22px caption strip. Until the desk is arranged, device templates are packed by `bestLayout` in `ui/src/lib/rig.ts` with a minimum of 3 cells of margin; once it is, they sit where the desk map puts them (`deskLayout`), with 12 cells of room around the desk while arranging; the default dot pitch is capped at 12px for all devices and 28px when zoomed into one.

**Vials:** a shelf switch (12px tabs, 18px apart) over six equal columns, 8px gap; each canvas is 160% of its cell wide (overhanging both neighbours) so glow, puddles and splashes fade out before any edge; height `clamp(118px, 14vw, 190px)`.

**Rhythm:** small steps of 2, 6, 8, 10 and 14px inside components; 18px between fields; 20px between main sections; 28px horizontal page inset; 36px between wide columns.

### Named Rules
**The Stage-First Rule.** The stage is the largest and brightest thing in every layout; controls never push it below the fold at the default 960 x 640 window.

**The Controls Grid Rule.** Anything placed in the controls area is one grid cell (a `.field`), or spans the row with `.wide`. Nested groups (More settings, zone sizes) inherit the parent's `--ctl-cols`, so they re-flow with it at every breakpoint.

## Elevation & Depth

Flat by default. Depth is tonal: ground, then rail, then one raise step, with 1px inset hairlines as edges. Soft, long, low-offset shadows appear only on the few things that float or are selected. There is no blur behind any surface, no translucency and no glass panel.

The only "lit" depth is the hardware light itself: on the dark stage, lit dots add a faint additive halo (16% at 1.75x radius); vials cast a coloured reflection and puddle on the table. Those are light, not elevation.

### Shadow Vocabulary
- **Selected plate** (`box-shadow: inset 0 0 0 1px var(--line), 0 6px 16px -12px var(--shadow)`): the gliding selection behind the chosen rail target.
- **Toast** (`box-shadow: inset 0 0 0 1px var(--line2), 0 18px 36px -18px var(--shadow)`): the floating notice.
- **Sheet** (`box-shadow: inset 0 0 0 1px var(--line2), 0 30px 60px -30px var(--shadow)`): the painter dialog, over a `--scrim` backdrop.
- **Knob** (`box-shadow: 0 2px 6px var(--shadow)`): slider thumb.
- **Swatch ring** (`box-shadow: 0 0 0 3px var(--ground), 0 0 0 4px var(--line2)`): colour inputs, a spaced hairline ring rather than a shadow.

### Named Rules
**The One Step Rule.** There is exactly one raised tone (`--raise`). Do not stack raised surfaces on raised surfaces; a sheet on the window is the only overlay.

## Shapes

Gently rounded, small-radius rectangles that scale with the object: 4px tags, 7px chips, 8px fields and icon buttons, 9px buttons, 10px rail plates, 11px toast, 12px wells, 14px sheet. Circles are reserved for things that are lights or knobs: LED dots, colour swatches, slider and switch knobs, the status dot. Keyboard keys in the painter are 5px rounded squares, one per real key unit.

Borders are 1px hairlines in `--line` (structure) or `--line2` (controls). Icons are drawn 24px SVGs, stroke-only at 1.7 with round caps and joins (`ui/src/lib/icons.ts`), shown at 14 to 18px.

### Named Rules
**The One Dot Rule.** There is one LED dot in the product. The logo's lit "f", the stage matrix and the rail's mini strips are all drawn from the same round dot at a regular pitch; new previews of light use it too, never bars, gradients in boxes or icons.

## Components

### Buttons
Quiet outlined controls; the content they act on is louder than they are.
- **Shape:** gently rounded (9px), 34px tall, 1px `--line2` border, transparent fill, 500 weight at 13.5px, 8px gap for an optional 14px icon.
- **Hover / Active / Focus:** hover fills with `--raise` and the border warms to `--muted`; press scales to 0.96; focus shows the global 2px lamp outline at 2px offset. Disabled is 45% opacity.
- **Small:** 28px tall, 12.5px text (Retry, Undo, Fill, Paint by hand).
- **Icon button:** 30px square, 8px radius, muted icon that turns to text colour on hover (theme switch, close).
- There is no filled primary button. The heading's actions (Arrange, Lights off, Stop all or Resume, Follow all devices) are normal buttons placed top right of the head; at 700px and below Arrange and Lights off keep only their icons.
- **Lights off** (power icon) sets one black look on every device, with Undo; unlike Stop all, devices do not go back to their own, usually lit, modes. While the lights are off (from here or the tray) the heading reads "Lights are off" ("Every device is dark. Pick a look to light them again."), the All devices card reads "Lights off", the button steps aside, the controls hide, and a look picked next starts from its own colours, not from black.

### Chips
- **Style:** 28px, 7px radius, `--line2` hairline, 12.5px text; used as toggle groups (built-in modes, a device's drawing when arranging) with `aria-pressed`.
- **State:** pressed fills with the lamp and lamp-ink text. A vertical 1px separator splits chips from a related action in the same row.

### Inputs / Fields
- **Field:** a label row (13px; value readout right-aligned in muted mono) over the control, 10px gap.
- **Slider:** 4px track in `--line2` with a lamp fill to the value (set through `--fill`), 16px bone knob that grows to 1.1x on hover and 1.25x while dragged.
- **Switch:** 34 x 20px; off track `--line2`, on track lamp with a lamp-ink knob; the knob stretches while pressed.
- **Colour:** 24px circle with the spaced hairline ring; the ring warms to `--muted` on hover.
- **Select:** 32px, 8px radius, hairline, custom chevron in muted.
- **Error:** an inline line in `--bad` with a faint `--bad` hairline, above the heading, announced as an alert.

### Navigation (the rail)
- Logo and wordmark, then a status line (6px status dot plus plain-language status; ellipsed, full text in the tooltip; a small Retry when failed; while connected a borderless rescan icon button at its end, "Look for devices again"). While OpenRGB looks for devices the line reads "Looking for devices…" (short form "Scanning 42%"), the dot pulses, the icon turns, and under it, in the space before "Targets" so nothing moves, a 2px `--line2` track fills with the lamp to the reported percent (a short lamp run sweeps it until the first report; still under reduced motion) above a mono line with the percent and the detector running. A rescan asked for here ends with a toast naming what came and went. An empty stage while connected offers "Look again". Then "Targets" (with a borderless `+` icon button, "New group of devices"): All devices first, then each group, then each device.
- Each target is a plate: name (13.5px, 500, text colour), LED count in mono, a muted subline (kind and vendor, or "Own look · <name>" in text colour behind a short lamp dash) and a mini strip of LED dots showing what the device is showing. Unselected strips sit at 60% opacity.
- A group's plate has the same anatomy: its connected device count, the subline "Group look · <name>" behind the lamp dash or "Group · follows all devices", and a strip of the group's look along its shared canvas. A device that follows its group's look says "In “<group>”".
- Selection is one raised plate that glides between targets (520ms) rather than each row toggling a background. Hover on others is a 55% raise wash.
- **Profiles** below the targets, after a `--line` hairline: a "Profiles" label with a borderless `+` icon button (save this look), then one 34px row per profile (13.5px name; clicking applies it, with Undo in the toast). The running profile carries the lamp-dash "On" tag, like "Own look". A borderless pencil icon button appears on hover or focus (always on touch) and opens the profile sheet. The list caps at 32vh and scrolls; with no profiles a muted line says how to make one.
- **Schedule** below the profiles, after a `--line` hairline: a "Schedule" label with a borderless pencil icon button ("Edit the schedule"), then one plate (whole plate opens the sheet; 55% raise wash on hover) listing up to four times in time order, each a row of mono `HH:MM`, the action (profile name or "Lights off", ellipsed) and the days right-aligned in muted ("Every day", "Weekdays", "Weekends", "Mon, Wed, Fri"), "and N more" past four, "No set times" when empty, and a muted line "Sleep: lights off · Wake: look from before". At 700px and below it is a 28px "Schedule" chip with a clock icon on its own row under the profile chips.
- **Window frame (Windows only):** the system title bar is off, and the app draws none as a strip: the rail and the ground run to the top edge, and the top 32px (`--frame-h`, set by `:root[data-frame]`) is a transparent drag region (double-click maximises). Minimise, Maximise/Restore and Close sit flush in the top-right corner as 44 x 32px borderless buttons with 15px icons in muted, raising to `--raise` and text colour on hover; Close stays neutral too (no red, per the Only Light Rule). Maximise is the Windows 11 snap-layout target (`#frame-max`). The main column's top padding grows to clear them; at 700px and below the top bar moves down by the same height. Linux keeps the system's decorated window.
- **App icon:** the logo's lit f with its unlit dots on a dark rounded tile in the rail colour with a `--line`-tone hairline (`crates/freergb-app/icons/icon.svg`); the tray uses the same icon.
- Theme switch and Settings (gear) pinned to the rail foot. At 700px and below the rail becomes the top bar described in Layout (with Settings beside the theme switch), the selected card carries its own raised fill, and profiles become a row of 28px apply-only chips under the targets.

### Stage (signature)
A canvas LED dot matrix (`ui/src/lib/Stage.svelte`) on which every device is drawn as a fixed integer-cell template from `ui/src/lib/rig.ts`: strip, memory stick, motherboard, keyboard (real key units), fans, ring. **The stage is the desk map.** Each device keeps its drawing (template, a strip's length in dots) and a spot on whole cells in one of four quarter turns, saved with the backend's desk map (`state.json`, `ui/src/lib/desk.ts`); the engine places each LED where the stage draws it (a fan's ring, a board's edge; keyboards with a key matrix keep their grid), so effects sweep across the room as drawn. Devices not placed yet follow in a row to the right.
- **Arrange view** (`ui/src/lib/ArrangePanel.svelte`, from "Arrange" in the all-devices heading, icon only at 700px and below): display heading "Arrange your desk", Cancel and Done top right, the stage with room around the desk, and below it the picked device's row (name and LED count, "Turn 90°", a strip's "Length on the map" slider, "Drawn as" chips); the vials step aside. Drag snaps to cells, and devices never overlap: a device moved, turned or redrawn onto another settles on the nearest free spot with at least one empty dot between them (`freeSpot` in `rig.ts`; a desk saved overlapping is pulled apart when arranging starts); the picked device's outline takes the lamp (mixed 80% with the outline colour, so it marks without outshining the light); the stage previews effects on the desk as it is being laid out. With the map focused (`role=application`): arrows move one dot, Shift four, R turns, Tab picks the next device. A device with no spot is labelled "· not placed yet". Done saves with Undo in the toast; the first arrangement starts from the packed rig.
- **Segments** (Arrange view): a picked device with a zone that is a run of LEDs offers "Split into segments" (split icon; "Edit segments" once split) beside "Turn 90°". It opens a sheet with the profile sheet's treatment: one hairline row per segment in wiring order (mono position, name field, LED count field with a muted "LEDs" unit, the run's first and last LED in muted mono, a remove icon), zone chips when the device has more than one such zone, "Add a segment" (named "Part n", taking what is left), and one status line ("LEDs 71–90 (20) stay with Desk strip.", "The segments use all 90 LEDs." or what is wrong, in text colour). The button reads "Split", "Save", or "Join back" when every segment is removed. Each segment becomes its own piece on the stage, named "Device · Segment", moved, turned and drawn by itself; new pieces start under the lowest piece of their device with room for its name. Hovering any piece outside Arrange lights up the whole device, and a click opens it. The toast confirms with Undo (the previous segments).
- Colours come from `ui/src/lib/preview.ts`, which mirrors the engine so the stage matches the hardware.
- All devices: the whole rig, device outlines in `--dot-line`, names in mono under each template (a name may run wider than its device; where it meets a neighbour's, both are shortened in the middle, and the hovered device's name shows whole; each name sits in a soft clearing where the dots fade out around it like a vignette, with no outline), hovered device outline brightens and the cursor becomes a pointer; clicking a device selects it (the rail is the keyboard path).
- One device: the camera flies to it, dots grow, others fade out, names hide.
- A group: the stage shows only its member devices, laid out left to right in group order; the caption names the group and its LED count. Clicking a member opens its own view.
- The matrix fades into the page at its edges (28px horizontal, 18px vertical mask) instead of sitting in a box. A mono caption strip below reads the target and its LED count.
- When there is nothing to light (starting, reconnecting, failed, no devices) the unlit matrix stays and a centred plain-language notice (with Retry when failed) sits on it.

### Vials (signature)
Numbered glass vials, one per look (`ui/src/lib/Vials.svelte`, drawn by `ui/src/lib/vial.ts`), each a button with the look name and a one-line description (or a tag) below. The looks sit on shelves of six (`SHELVES` in `ui/src/lib/effects.ts`): **Essentials** (static colour, gradient, spectrum cycle, rainbow, breathing, wave), **Scenes** (comet, marquee, bouncing beam, starfield, fire, rain) and **My effects**, the user's kept looks: one vial per saved effect in its own colours (not tinted by the running look), named by the user with the effect type as its description, in rows of six. On that shelf the tab row carries two small buttons on its right, "Save this look" (disabled until a software look is playing on the target, and while that look is already kept) and "Import…"; a borderless pencil at each saved vial's top left (on hover or focus, always on touch) opens the effect sheet. Empty, the shelf says in one muted line how to fill it. A saved vial is "Playing" when the target's look is exactly that effect, and the shelf then stays open instead of jumping to the effect's kind; the lamp dot then marks My effects, not the kind's shelf. A row of text tabs above the rack switches shelf (muted label; the open one in text colour with a 14px lamp rule under it; arrow keys, Home and End move between tabs). The shelf holding the playing look carries a 5px lamp dot while another shelf is open, and opens by itself when a look on it starts playing elsewhere (a profile, the tray). Opening a shelf deals its vials in (500ms, 30ms steps). Number keys 1 to 6 pick from the open shelf; 0 returns a device to following all devices (`ui/src/lib/shortcut.ts`). A vial's liquid shows the look smoothly (`glow` in `ui/src/lib/preview.ts`): each of its nine colour stops averages a little of the canvas around it, and its colour and level ease over a quarter second. It holds its colour through dark phases instead of snapping, so sparse looks (starfield, rain, comet) and breathing do not flicker in the rack.
- A look's density slider is named for what it sets: Density (rainbow, wave), Comets, Dashes, Focus (bouncing beam: higher is narrower), Stars, Detail (fire), Drops.
- Choosing a look carries the running look's colours over, except fire, which starts from its own embers-to-yellow palette unless it is already playing.
- The liquid is the look's own light, tinted with the current look's colours and speed.
- Levels: resting 60%, hovered or focused 80%, playing brims over (dome, two runs down the glass, drops, puddle ripples on the table).
- The vial nearest the pointer tilts slightly with pointer motion; its liquid lags, sloshes and can spill over the rim.
- Tags under the playing vial: "Playing" (all devices) or "Own look" (device view) in lamp; the look a device would otherwise follow is marked with a ghost "Shared" tag.

### Direction dial
`ui/src/lib/Dial.svelte`, a field in the controls grid for effects that run in a direction (gradient, rainbow, wave, comet, marquee, bouncing beam, fire, rain): the slider's parts bent round, a 46px ring of the 4px `--line2` track with a lamp needle from the centre and the bone knob riding the ring. Drag or arrow keys in 15° steps (Page keys 45°), `role=slider` with the angle and its compass word as value text; 0° runs right, 90° down. The readout is mono degrees. When set, a small button returns to the effect's own direction, named for it ("Left to right", "Fall down", "Rise up"). Reverse (in More settings) runs an aimed look the other way, so with it on the dial, its readout and the stage arrow show where the look really moves (turned 180°); the return button also clears Reverse. Turning it draws a short arrow across the middle of the stage along the new direction: a smooth stroked line in `--dot-glow` drawn freely over the matrix, not on its dots (the user asked for it off the grid: it is a pointer, not light), fading in from the tail with a soft bead running to its head, gone in 1.1 s (no bead under reduced motion).

### Follows
A field in a software look's controls, shown once this machine offers something to follow (`ui/src/lib/Controls.svelte`, words in `ui/src/lib/follow.ts`): "Follows" with the live reading as a mono readout after the label ("64 °C", "37%", "Live"; "No reading yet" in muted when there is none; a source that needs another program or a permission says so in one muted line under the select, "Needs LibreHardwareMonitor running.", or on Wayland "Your desktop asks which screen to share."), and a select: Nothing, then the sources this machine has (CPU or GPU temperature or load, Sound level, Sound beat, Sound spectrum, Screen colours); a source the look cannot use is disabled. Picking one adds "Moves", chips for the settings that reading can move on this look (Brightness, Colour, and the look's own amount named as in More settings: Comets, Stars, Drops, Density); a temperature or the screen starts on Colour, the rest on Brightness. The screen and the spectrum move one thing each, said in one muted line instead of chips. While a reading moves the colour the colours field reads "Colours, low to high" and offers at least two; following the screen it reads "From the screen". The sound readings (level, beat, spectrum) add two sliders after Moves, as percentages: "Sensitivity" (50% is the reading as it is; lower damps it, higher amplifies it) and "Strength" (how far the setting swings; under 100% the lights no longer go fully dark or travel the whole palette); the usual values are stored as unset. "Reading range" (two mono number fields, the usual range as placeholders, blank goes back to it) sits in More settings. The stage, the rail strips and the vials draw the look with the live readings (sent at most 15 times a second while the window is open), so the preview stays the room.

### Rename or hide
"Rename or hide" (pencil; icon only at 700px and below) sits in a device's heading next to "Follow all devices". It opens a sheet with the profile sheet's treatment (`ui/src/lib/DeviceSheet.svelte`): headline, a muted line saying the name is freeRGB's own and OpenRGB keeps the device's name, a name field with the device's own name as placeholder and the mono `n/40` count (blank goes back to its own name), and a "Hide from freeRGB" switch row with a description. The submit button reads "Hide" when the sheet hides the device. A user's name replaces the device's name everywhere (rail, stage, heading, group sheet); identical devices without one stay numbered. A hidden device leaves the rail, the stage and every look, and the window returns to all devices; the toast says where to bring it back, with Undo. Settings lists hidden devices under "Hidden devices" (hairline rows, "Not connected now" when away) with a small "Show" button each; a group member that is hidden reads "Hidden" in the group sheet. If every device is hidden, the stage says so and offers Settings.

### Save to device
Shown under a built-in mode's settings only when the device can keep that mode in its own memory (OpenRGB's `MANUAL_SAVE`): a small button with a down-into-tray icon, then one muted line saying what it does. The button sends any slider edit still waiting first, then saves. A toast confirms; there is no Undo, because the device itself now holds the mode.

### Zone modes
Only on devices whose zones have built-in modes of their own (OpenRGB 1.1 per-zone modes: fan hubs such as NZXT Hue 2 or a Corsair Lighting Node), and only while the device runs one of its built-in modes. A "Zones" chip row tops the controls: "Whole device" and one chip per zone; a zone with a mode of its own shows that mode's name after its name at 70% opacity. Picking a zone swaps the controls and the mode chips for the zone's ("Modes of Fan 2", starting with "Same as device"); a zone that follows the device says so in one muted line. Save to device stays with "Whole device". Under a mode that drives the whole device (`REQUIRES_ENTIRE_DEVICE`) the row gives way to one muted line, and the zones' own modes are kept in the look for when another mode is picked.

### About this device
A Fold labelled "About this device" at the end of a device's controls: a hairline-ruled list of what OpenRGB reports (type, vendor, description, version, serial, location; empty fields left out), labels muted at 13px, values in text colour with version, serial and location in mono; the zones row lists each zone with its LED count (and grid size for a keyboard) in muted mono. Values are selectable so a serial or location can be copied.

### More settings (Fold)
A text toggle with a rotating chevron opening a drawer that glides open by animating grid rows (520ms); content fades and lifts in 80ms later. When closed the content stays laid out but is `inert`, so it takes no focus. Its inner grid follows the controls grid.

### Toast
A raised notice centred over the main column, 18px from the bottom, rising in on open. A 2px lamp line along its bottom drains over 7 s, then the toast closes. Hover or focus pauses the drain so Undo stays reachable; Ctrl+Z (Cmd+Z) triggers Undo while it is offered, except while typing in a field or with a dialog open. Every look change and Stop all goes through it with Undo.

### Painter sheet
A modal `<dialog>` with the matte sheet treatment over a warm scrim: header (headline plus muted explanation, close icon), a brush row of round swatches (selected swatch gets a lamp ring, "off" is a struck-through hollow circle, a custom colour picker), then one recessed `--stage` well per zone (keyboard zones as a grid of rounded keys in real key units, other zones as a row of round LEDs), and a footer with Cancel and Done. LEDs use a crosshair cursor and paint live on the device; Cancel restores.

### Profile and Settings sheets
Small (440px) sheets with the painter's treatment: headline, muted explanation, close icon. The profile sheet has a name field (34px, `--ground` fill, hairline, mono `n/40` count) with a hint line under it that says when a save replaces a profile of the same name (the button then reads "Replace") or a rename is taken; editing adds a switch to replace the profile's look with the current one, and a Delete that asks once more in place ("Delete for good") instead of opening a confirmation. The Settings sheet holds labelled rows with a muted description: a switch for starting at login, "Animation smoothness" with two chips (30 fps, 60 fps; the pressed one in lamp, numbers as readouts) that apply at once, "Diagnostics" with a small "Open" button that closes Settings and opens the diagnostics sheet in its place (one sheet at a time), and "Start over" with a small "Reset" button that asks once more in place (the description turns into what will be gone and "This cannot be undone.", the buttons into Cancel and "Reset for good"); a reset forgets everything freeRGB keeps, stops effects and starting at login, leaves OpenRGB's settings alone and reloads the window. Then plain notes about tray behaviour.

### Diagnostics sheet
`ui/src/lib/DiagnosticsSheet.svelte`, the sheet treatment at a fixed 820 x 640px (smaller on small windows) so switching tabs or a filling log never moves it. Header: headline and a muted line saying it is for looking into a problem and changes no lights. Then the vial shelves' text tabs, **Log** and **Hardware**, with the open tab's small buttons on the right of the same row. Each tab is one recessed `--stage` well with a `--line` inset hairline that scrolls inside.
- **Log:** OpenRGB's lines in Geist Mono 12px (log text is data): time since OpenRGB started (`m:ss.mmm`, right-aligned, muted), the level word (muted; warnings and errors in text colour behind the status dot) and the message in text colour, wrapping. The well keeps to the bottom while new lines arrive unless scrolled up. A 28px "Show" select (Problems, Usual, Everything; Usual hides verbose, debug and trace), Copy (the shown lines as plain text; the label reads "Copied" for 1.6 s) and Clear. At 700px and below only warnings and errors keep their level word (beside the dot, so the level never rests on colour alone); the usual levels drop theirs for screen readers only. When OpenRGB keeps no log, the well is replaced by a plain statement at the left: a Title ("OpenRGB is not keeping a log"), a muted explanation of the "Enable Log Console" setting and what turning it on does here, and a normal button "Turn on OpenRGB’s log". An OpenRGB freeRGB started is restarted ("Restarting OpenRGB…"); another one says to restart it ("Restart OpenRGB to start its log"). Without a local connection it says the log needs one.
- **Hardware:** what OpenRGB sees, in four sections (SMBus buses, USB devices, HID devices, Serial ports), each a 13.5px title with its count as a muted readout over hairline rows: the device's own name (or "Unnamed") with a muted mono subline where useful (HID usage, interface and path; a serial port's USB port), and its ids right-aligned in muted mono as the OS writes them (`1B1C:0C1A`; PCI ids with the subsystem in brackets). Empty sections say "None found." "Look again" (rescan icon) asks once more; Copy gives the whole list as plain text.

### Schedule sheet
`ui/src/lib/ScheduleSheet.svelte`, the profile sheet's treatment and footer (Cancel, Save; edits apply on Save). Header explains it runs from the tray and that a time missed while closed does not fire later. "At set times" is a hairline list of rules in time order: a native time field (mono, `--ground` fill, wide enough for AM/PM), a select (Lights off, then an "Apply a profile" group) and a borderless remove icon; under it seven 26px day chips (M T W T F S S, full day names as labels; pressed in lamp). A rule with no day says "Pick a day" and disables Save. "Add a time" (small button, plus icon) adds 23:00 Lights off first, then an hour after the last. Then two labelled rows with selects: "When the PC sleeps" (Lights off, Nothing, profiles) and "When it wakes" (Look from before, Nothing, profiles). A profile that was deleted elsewhere still shows as the value instead of silently changing.

### Group panel and sheet
A group's view (`ui/src/lib/GroupPanel.svelte`) follows the all-devices panel: the group's name as the display heading, a subline ("3 devices as one surface", or that it follows all devices and how to give it a look), "Follow all devices" (key 0, only while it has a look) and "Edit group" (pencil; icon only at 700px and below so both stay on the heading row), the stage, the vials ("Group look" tag; the all-devices look marked "Shared") and the controls. Picking a look comes with Undo; members' own looks stay on top.
The group sheet (`ui/src/lib/GroupSheet.svelte`) has the profile sheet's treatment: name field with the mono count; "In this group, left to right" as a hairline list of rows (mono position, name, kind and LED count, then borderless up, down and remove icon buttons; focus stays on the arrow as a row moves; a member that is not connected shows "Not connected now" and keeps its place); "Add devices" lists the rest with a small "Add" button, disabled with "In “<group>”" for a device already in another group. "Create group" / "Save" needs a name and two devices; Delete asks once more in place.

### Effect sheet
The profile sheet's treatment (`ui/src/lib/EffectSheet.svelte`). Keeping a look suggests the effect's name ("Fire", then "Fire 2"); a taken name turns Save into "Replace". Editing offers a switch to replace it with the look that is playing, a hairline-separated "Share it as a file…" row with "Export…" (a native save dialog; the toast says where the file went), and Delete that asks once more in place. Import uses a native open dialog; a name already taken gets a number.

### Motion grammar
- **Easing:** CSS uses `--ease` (`cubic-bezier(0.16, 1, 0.3, 1)`, a strong ease-out); canvas tweens (`ui/src/lib/tween.ts`) use an ease-out quart. Nothing bounces.
- **Durations:** `--fast` 180ms (hover, colour, press), `--mid` 300ms (switch, chevron, tag pop), `--slow` 520ms (content arriving, the rail plate glide, the drawer). Canvas: look wipe 650ms, device fade 420ms in and 700ms back out, camera 760ms, vial fill 1100ms (playing) and 600ms (others), lamp warm-up 1200ms when the rig first has something to show, theme reveal 700ms.
- **Arrival:** content rises 10px out of a slight blur and fades in; boot staggers the rail and heading (40 to 60ms steps) and deals the vials in with a small rise and rotation.
- **Look change:** the new look wipes across the matrix left to right over the old one while its vial fills and overflows.
- **Theme switch:** a circular reveal spreading from the switch (View Transitions), instant where unsupported.
- **Pointer glow:** on the stage, dots within about six cells of the pointer warm towards `--dot-glow` (bone in dark, a deep warm grey in light) and grow up to 1.45x, fading out with distance; the glow trails the pointer slightly, rises in 300ms and fades out 520ms after the pointer leaves.
- **Ambient:** only the logo (hue drift 8 s, chase pulse 2.6 s) and the pulsing status dot while starting or reconnecting move without user input, besides the live light itself.
- **Reduced motion:** still frames. All CSS animations and transitions are switched off, previews render at t = 0 (still, coloured), vials do not tilt or slosh, and the theme switches instantly. Canvas tweens (camera zoom, device fade, vial levels, warm-up) snap to their targets (`ui/src/lib/tween.ts`) and a look change swaps without the wipe.

### Named Rules
**The Brightest Thing Rule.** The selected device is the brightest thing on screen; everything else dims around it, chrome included.

**The Undo Everywhere Rule.** Any action that changes what the hardware shows across devices (a look, Stop all) is immediate and comes with Undo in the toast, never behind a confirmation dialog.

## Do's and Don'ts

### Do:
- **Do** use only the custom properties in `ui/src/app.css`; add a token there (with a light-theme value) before using a new colour.
- **Do** keep the lamp as the single accent for selection, on-state, progress and focus, and let it turn to ink in the light theme.
- **Do** render any preview of light with the shared LED dot (canvas, `ui/src/lib/frames.ts` loop, colours from `ui/src/lib/preview.ts`) so it matches the stage and the hardware.
- **Do** set counts and readings in Geist Mono with tabular figures (`.num`).
- **Do** separate with 1px hairlines and the one raise step; put new controls in the controls grid as `.field` cells and deeper settings in a Fold.
- **Do** give every cross-device change an Undo through the toast.
- **Do** write status and empty states in plain language, on the stage, with the action (Retry) next to them.
- **Do** honour reduced motion by rendering still frames, not by hiding content.

### Don't:
- **Don't** add hue to chrome: no coloured panels, gradients, accent buttons or tinted backgrounds. Saturation belongs to the light.
- **Don't** use glass as a surface: no `backdrop-filter`, translucent panels or frosted cards. Glass exists only as a drawn object (the vials).
- **Don't** build lists of equal device cards or a knobs-and-dials form; devices live on the stage and in the rail.
- **Don't** introduce a second accent colour, a filled primary button, or a second raised tone.
- **Don't** use uppercase or letter-spaced labels, eyebrow text above headings, or icon glyphs from fonts; icons are the drawn stroke set in `ui/src/lib/icons.ts`.
- **Don't** load fonts or images from remote URLs or `data:` URLs; the CSP blocks them. Bundle assets.
- **Don't** use hard offset or dark heavy shadows; shadows are long, soft and low-offset, and only on floating or selected things.
- **Don't** let any control or overlay outshine the stage.
