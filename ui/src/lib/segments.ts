// Splitting a zone into segments: the runs the sheet edits and what it says about them.

import type { Device, Zone } from './api.ts'

export interface Run {
  name: string
  leds: number
}

export const MAX_RUNS = 32
export const MAX_RUN_NAME = 40

/** Zones that can be split: a run of LEDs, not a keyboard's key grid. */
export const splittable = (d: Device): Zone[] => d.zones.filter((z) => !z.matrix && z.leds >= 2)

/** The zone's segments as runs, in order (what the sheet starts from). */
export const runsOf = (d: Device, zone: number): Run[] =>
  (d.pieces ?? []).filter((p) => p.segment?.zone === zone).map((p) => ({ name: p.name, leds: p.leds.length }))

/** Runs for a zone not split yet: a first half, the rest staying with the device. */
export const firstRuns = (z: Zone): Run[] => [{ name: 'Part 1', leds: Math.max(1, Math.floor(z.leds / 2)) }]

/** The next run to add: named after its place, taking what is left (at least one LED). */
export function nextRun(runs: Run[], z: Zone): Run {
  const left = z.leds - runs.reduce((a, r) => a + r.leds, 0)
  return { name: `Part ${runs.length + 1}`, leds: Math.max(1, left) }
}

export type RunCheck =
  | { ok: true; rest: number }
  | { ok: false; why: string }

/** Whether `runs` fit `z`: named, whole LED counts of at least one, in all no more than it has. */
export function checkRuns(runs: Run[], z: Zone): RunCheck {
  if (runs.length > MAX_RUNS) return { ok: false, why: `A zone holds at most ${MAX_RUNS} segments.` }
  const i = runs.findIndex((r) => r.name.trim() === '')
  if (i >= 0) return { ok: false, why: `Segment ${i + 1} needs a name.` }
  if (runs.some((r) => r.name.trim().length > MAX_RUN_NAME)) return { ok: false, why: `Keep names under ${MAX_RUN_NAME} characters.` }
  if (runs.some((r) => !Number.isInteger(r.leds) || r.leds < 1)) return { ok: false, why: 'Each segment needs at least one LED.' }
  const used = runs.reduce((a, r) => a + r.leds, 0)
  if (used > z.leds) return { ok: false, why: `That is ${used - z.leds} LEDs more than ${z.name} has (${z.leds}).` }
  return { ok: true, rest: z.leds - used }
}

/** Each run's first and last LED, counted from 1 as people do. */
export function spans(runs: Run[]): [number, number][] {
  let at = 0
  return runs.map((r) => {
    const s: [number, number] = [at + 1, at + Math.max(r.leds, 0)]
    at += Math.max(r.leds, 0)
    return s
  })
}
