# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users
Mostly casual PC owners with RGB hardware from several vendors (case fans, RAM, motherboard, peripherals). They want a good-looking setup in a few clicks, then forget the app: it lives in the tray and the window is opened occasionally to change a look or fix a device. Simplicity comes first. Power users are served by the same app through progressive disclosure: every detailed or advanced setting stays reachable, never blocked by the simple default view.

## Product Purpose
A lightweight, modern desktop front-end for RGB lighting, built as a separate client of the OpenRGB SDK server (not a fork of its Qt app). Goal: feature parity with the OpenRGB app, with a far better UI and a very small footprint in the tray. Success: a casual user sets an effect for all devices, or a different one per device, without reading docs, while an advanced user can still reach per-mode settings, per-zone and per-LED control.

## Positioning
The OpenRGB hardware coverage with a calm, approachable interface and a tray-resident engine that costs almost nothing when idle. A neighbouring vendor suite cannot truthfully claim support for hardware from every vendor; this one inherits OpenRGB's.

## Operating Context
- Tauri desktop app (Windows 11 and Linux). Closing the window destroys the webview; the Rust backend keeps running effects from the tray.
- The backend owns or adopts an OpenRGB server; connection states are Starting, Connected, Reconnecting, Failed.
- Effects are declarative scenes (effect plus targets plus canvas) rendered by the Rust engine; the UI designs them and shows a live preview.
- The window is a rail (connection, targets, profiles, schedule) beside a stage: the desk map with the live preview, the effect shelves, and the look's settings.

## Capabilities and Constraints
- Twelve software effects for all devices, a group or one device; built-in device modes per zone; painting LEDs by hand on a desk map; profiles with tray switching; a schedule; looks that follow temperature, load, sound or the screen.
- Dark-first; light theme available.
- The name is freeRGB.
- Device identity is by OpenRGB name, location, serial; two identical devices must stay distinguishable.

## Brand Commitments
- Minimal, muted and matte (decided 2026-11-10, replacing the earlier "liquid glass" commitment): warm near-black surfaces, hairlines, no glass or coloured backdrops; the light the hardware makes is the only saturated colour on screen.

## Evidence on Hand
No logo, brand assets or user research. Real device data exists only as OpenRGB captures in the repo fixtures and the mock server; designs must label any demonstration devices as synthetic.

## Product Principles
- Simple by default, deep on demand: the first screen needs no explanation, advanced controls are one step away rather than absent.
- Show the light: previews and device colours are the content; the interface frames them.
- Honest state: connection, detection and failure states are stated in plain language.
- Cheap to leave running: the UI is for visits; nothing about it should imply a heavy always-on app.
