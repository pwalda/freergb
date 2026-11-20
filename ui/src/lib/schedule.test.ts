import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  actionOf,
  actionValue,
  byTime,
  clock,
  timeText,
  daysLabel,
  emptySchedule,
  EVERY_DAY,
  minutes,
  nextRule,
  powerLine,
  WEEKDAYS,
} from './schedule.ts'

test('times read and write as HH:MM', () => {
  assert.equal(clock(425), '07:05')
  assert.equal(clock(23 * 60), '23:00')
  assert.equal(minutes('7:05'), 425)
  assert.equal(minutes('24:00'), null)
  assert.equal(minutes('12:60'), null)
  assert.equal(minutes('noon'), null)
})

test('times shown to read follow the locale’s clock', () => {
  assert.equal(timeText(18 * 60 + 30, 'en-GB'), '18:30')
  assert.match(timeText(18 * 60 + 30, 'en-US'), /^06:30\s?PM$/)
})

test('days read as people say them', () => {
  assert.equal(daysLabel(EVERY_DAY), 'Every day')
  assert.equal(daysLabel(WEEKDAYS), 'Weekdays')
  assert.equal(daysLabel(0b110_0000), 'Weekends')
  assert.equal(daysLabel(0b000_0010), 'Tuesday')
  assert.equal(daysLabel(0b001_0101), 'Mon, Wed, Fri')
})

test('the rail line names sleep and wake', () => {
  assert.equal(powerLine(emptySchedule()), 'Sleep: lights off · Wake: look from before')
  assert.equal(
    powerLine({ rules: [], sleep: { kind: 'nothing' }, wake: { kind: 'profile', name: 'Calm' }, shutdown: { kind: 'off' } }),
    'Sleep: nothing · Wake: Calm',
  )
})

test('actions round-trip through select values', () => {
  for (const a of [{ kind: 'off' }, { kind: 'nothing' }, { kind: 'back' }, { kind: 'profile', name: 'p:x' }] as const)
    assert.deepEqual(actionOf(actionValue(a)), a)
})

test('the next rule keeps to its days and wraps the week', () => {
  // Monday 16 November 2026, 22:00 local.
  const now = new Date(2026, 10, 16, 22, 0)
  const s = {
    ...emptySchedule(),
    rules: [
      { at: 8 * 60, days: WEEKDAYS, action: { kind: 'profile', name: 'Work' } as const },
      { at: 23 * 60, days: 0b100_0000, action: { kind: 'off' } as const },
    ],
  }
  const n = nextRule(s, now)!
  assert.equal(n.rule.at, 8 * 60)
  assert.equal(n.at.getDate(), 17)
  // Sunday-only rule from Monday is six days on.
  const sun = nextRule({ ...s, rules: [s.rules[1]] }, now)!
  assert.equal(sun.at.getDate(), 22)
  assert.equal(nextRule(emptySchedule(), now), null)
  assert.deepEqual(byTime(s.rules.slice().reverse()).map((r) => r.at), [480, 1380])
})
