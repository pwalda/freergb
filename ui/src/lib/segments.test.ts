import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { Device, Zone } from './api.ts'
import { checkRuns, firstRuns, nextRun, runsOf, spans, splittable } from './segments.ts'

const zone = (leds: number, matrix = false): Zone => ({
  index: 0, name: 'Strip', leds, min_leds: leds, max_leds: leds, resizable: false, start: 0,
  matrix: matrix ? { width: 1, height: 1, cells: [0] } : null, modes: [], active_mode: -1,
})

test('runs must be named, hold an LED each and fit the zone; the rest stays with the device', () => {
  const z = zone(60)
  assert.deepEqual(checkRuns([{ name: 'Desk', leds: 20 }, { name: 'Shelf', leds: 30 }], z), { ok: true, rest: 10 })
  assert.deepEqual(checkRuns([], z), { ok: true, rest: 60 })
  assert.equal(checkRuns([{ name: ' ', leds: 2 }], z).ok, false)
  assert.equal(checkRuns([{ name: 'A', leds: 0 }], z).ok, false)
  assert.equal(checkRuns([{ name: 'A', leds: 2.5 }], z).ok, false)
  const over = checkRuns([{ name: 'A', leds: 50 }, { name: 'B', leds: 20 }], z)
  assert.ok(!over.ok && over.why.includes('10 LEDs more'))
})

test('a fresh split starts with half; added runs take what is left; spans count from 1', () => {
  const z = zone(61)
  assert.deepEqual(firstRuns(z), [{ name: 'Part 1', leds: 30 }])
  assert.deepEqual(nextRun(firstRuns(z), z), { name: 'Part 2', leds: 31 })
  assert.deepEqual(spans([{ name: 'A', leds: 30 }, { name: 'B', leds: 31 }]), [[1, 30], [31, 61]])
})

test('only runs of LEDs split, and a zone’s segments come back as runs', () => {
  const d = { zones: [zone(60), zone(4, true), zone(1)], pieces: [
    { segment: null, name: '', leds: [0] },
    { segment: { zone: 0, start: 0 }, name: 'Desk', leds: [1, 2] },
  ] } as unknown as Device
  assert.equal(splittable(d).length, 1)
  assert.deepEqual(runsOf(d, 0), [{ name: 'Desk', leds: 2 }])
  assert.deepEqual(runsOf(d, 1), [])
})
