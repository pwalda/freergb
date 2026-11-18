// The words and numbers of the "Follows" control.

import type { Drive, EffectKind, SourceId } from './effects.ts'
import { canFollow } from './effects.ts'
import { RANGE, valueOf, type Readings } from './react.ts'

export const SOURCE_LABEL: Record<SourceId, string> = {
  cpu_temp: 'CPU temperature',
  gpu_temp: 'GPU temperature',
  cpu_load: 'CPU load',
  gpu_load: 'GPU load',
  sound: 'Sound level',
  beat: 'Sound beat',
  spectrum: 'Sound spectrum',
  screen: 'Screen colours',
}

export const DRIVE_LABEL: Record<Drive, string> = { brightness: 'Brightness', color: 'Colour', amount: 'Amount' }

/** The unit a reading is shown in. */
export const unitOf = (s: SourceId) => (s === 'cpu_temp' || s === 'gpu_temp' ? '°C' : '%')

/** Whether the reading is one number with a range worth setting. */
export const ranged = (s: SourceId) => s !== 'spectrum' && s !== 'screen'

/** A reading as people read it ("62 °C", "48%"); sound and beat (0..1) as a percentage. */
export function readout(s: SourceId, r: Readings): string | null {
  if (s === 'spectrum') return r.spectrum?.length ? 'Live' : null
  if (s === 'screen') return r.screen ? 'Live' : null
  const v = valueOf(r, s)
  if (v === undefined) return null
  return fraction(s) ? `${Math.round(v * 100)}%` : unitOf(s) === '°C' ? `${Math.round(v)} °C` : `${Math.round(v)}%`
}

/** Readings kept as 0..1 and shown as percentages. */
const fraction = (s: SourceId) => s === 'sound' || s === 'beat'

/** The sound readings, which offer sensitivity and strength. */
export const isSound = (s: SourceId) => s === 'sound' || s === 'beat' || s === 'spectrum'

/** The range in the unit shown (sound's 0..1 as 0..100). */
export const shown = (s: SourceId, v: number) => (fraction(s) ? Math.round(v * 100) : v)
export const stored = (s: SourceId, v: number) => (fraction(s) ? v / 100 : v)

export const usualRange = (s: SourceId) => RANGE[s]

/** The settings `source` can move on `kind`, in the order offered. */
export const drivesFor = (kind: EffectKind, source: SourceId): Drive[] =>
  (['brightness', 'color', 'amount'] as Drive[]).filter((d) => canFollow(kind, source, d))

/** What picking `source` moves first: colour for a temperature or the screen, else brightness. */
export function firstDrive(kind: EffectKind, source: SourceId): Drive | null {
  const can = drivesFor(kind, source)
  const want: Drive = source === 'cpu_temp' || source === 'gpu_temp' || source === 'screen' ? 'color' : 'brightness'
  return can.includes(want) ? want : (can[0] ?? null)
}
