import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import type { Device } from './api.ts'
import { scanDetail, scanOutcome, scanShort } from './detection.ts'

const dev = (name: string, nth = 0) => ({ name, key: { name, location: 'L', serial: '', nth } }) as Device

test('the status reads the scan with or without progress', () => {
  assert.equal(scanDetail({ scanning: true, percent: null, label: '' }), '')
  assert.equal(scanDetail({ scanning: true, percent: 42, label: 'Corsair' }), '42% · Corsair')
  assert.equal(scanShort({ scanning: true, percent: 7, label: '' }), 'Scanning 7%')
})

test('the outcome names what came and what went', () => {
  const a = dev('RAM'), b = dev('K70'), c = dev('Fan')
  assert.equal(scanOutcome([a], [a]), 'Scan finished. No new devices.')
  assert.equal(scanOutcome([a], [a, b]), 'Found a new device: K70.')
  assert.equal(scanOutcome([a], [a, b, c]), 'Found 2 new devices.')
  assert.equal(scanOutcome([a, b], [a]), '1 device is gone.')
  assert.equal(scanOutcome([a, b], [c]), 'Found a new device: Fan; 2 devices are gone.')
  // Two identical sticks are told apart by their key.
  assert.equal(scanOutcome([a], [a, dev('RAM', 1)]), 'Found a new device: RAM.')
})
