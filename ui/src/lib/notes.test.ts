import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import type { Device, DeviceNote } from './api.ts'
import { displayNames, shownDevices } from './notes.ts'

const dev = (name: string, nth = 0) => ({ name, key: { name, location: 'L', serial: '', nth } }) as Device

test('a name of the user’s wins; identical devices without one stay numbered', () => {
  const sticks = [dev('RAM', 0), dev('RAM', 1), dev('RAM', 2)]
  assert.deepEqual(displayNames(sticks, []), ['RAM #1', 'RAM #2', 'RAM #3'])
  const notes: DeviceNote[] = [{ device: sticks[0].key, name: 'Top stick', hidden: false }]
  assert.deepEqual(displayNames(sticks, notes), ['Top stick', 'RAM #2', 'RAM #3'])
  // Once only one is left without a name of its own, it needs no number.
  notes.push({ device: sticks[1].key, name: 'Middle', hidden: false })
  assert.deepEqual(displayNames(sticks, notes), ['Top stick', 'Middle', 'RAM'])
})

test('hidden devices are left out', () => {
  const a = dev('Strip'), b = dev('Fan')
  assert.deepEqual(shownDevices([a, b], [{ device: b.key, hidden: true }]), [a])
  assert.deepEqual(shownDevices([a, b], [{ device: b.key, name: 'Rear', hidden: false }]), [a, b])
})
