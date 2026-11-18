import type { ModeView } from './api.ts'
import { DEFAULT_PARAMS, type SceneParams, type ZoneLook } from './effects.ts'

/** How many colour pickers a built-in mode shows: the fewest it needs, at least one if it takes any. */
export const modeColorCount = (m: ModeView) =>
  m.colors_max === 0 ? 0 : Math.min(Math.max(m.colors_min, 1), m.colors_max)

/** Settings that start a built-in mode from what the device itself reports for it, so choosing a
 *  mode does not make the device jump to values the user never saw. */
export function modeStart(m: ModeView): SceneParams {
  const n = modeColorCount(m)
  const colors = Array.from({ length: n }, (_, i) => m.colors[i] ?? m.colors.at(-1) ?? '#ffffff')
  return {
    ...DEFAULT_PARAMS,
    kind: 'device_mode',
    mode: m.name,
    colors,
    speed: m.speed ?? 0.5,
    brightness: m.brightness ?? 1,
    modeDirection: m.direction,
  }
}

/** Option values in the effect picker: software effects are their kind, built-in modes are `mode:<name>`. */
export const MODE_PREFIX = 'mode:'
export const modeValue = (name: string) => MODE_PREFIX + name
export const modeNameOf = (value: string) =>
  value.startsWith(MODE_PREFIX) ? value.slice(MODE_PREFIX.length) : undefined

/** A zone's own mode, starting from what the device reports for it (like `modeStart`). */
export function zoneLookOf(zone: number, m: ModeView): ZoneLook {
  return lookOfParams(zone, modeStart(m))
}

/** The settings a zone look shows in `ModeControls`. */
export function zoneParams(l: ZoneLook): SceneParams {
  return {
    ...DEFAULT_PARAMS,
    kind: 'device_mode',
    mode: l.mode,
    colors: [...l.colors],
    speed: l.speed,
    brightness: l.brightness,
    modeDirection: l.mode_direction ?? null,
  }
}

/** `ModeControls`' settings back as the zone's look. */
export function lookOfParams(zone: number, p: SceneParams): ZoneLook {
  return {
    zone,
    mode: p.mode ?? '',
    colors: [...p.colors],
    speed: p.speed,
    brightness: p.brightness,
    ...(p.modeDirection ? { mode_direction: p.modeDirection } : {}),
  }
}

/** `looks` with `zone`'s entry replaced by `look`, or dropped (the zone follows the device's
 *  mode) when `look` is null. Kept in zone order so the same setup always sends the same scene. */
export function withZoneLook(looks: ZoneLook[] | undefined, zone: number, look: ZoneLook | null): ZoneLook[] {
  const rest = (looks ?? []).filter((l) => l.zone !== zone)
  return (look ? [...rest, look] : rest).sort((a, b) => a.zone - b.zone)
}
