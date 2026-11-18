import { test } from 'node:test'
import assert from 'node:assert/strict'
import { deviceKind } from './device-kind.ts'
import { ICONS } from './icons.ts'
import { resolveTheme } from './theme.ts'

test('every known device type has a label and a drawn icon, and unknown ones fall back', () => {
  for (let t = 0; t <= 20; t++) {
    const k = deviceKind(t)
    assert.ok(k.label.length > 0)
    assert.ok(k.icon in ICONS, `type ${t} names a missing icon ${k.icon}`)
  }
  assert.deepEqual(deviceKind(1), { label: 'Memory', icon: 'ram' })
  assert.equal(deviceKind(99).label, 'Device')
  assert.equal(deviceKind(-1).icon, 'chip')
})

test('a stored theme wins over the system; otherwise follow it, dark when unsure', () => {
  assert.equal(resolveTheme('light', false), 'light')
  assert.equal(resolveTheme('dark', true), 'dark')
  assert.equal(resolveTheme(null, true), 'light')
  assert.equal(resolveTheme('garbage', false), 'dark')
})
