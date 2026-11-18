// The schedule (mirrors `schedule.rs`): times of day that turn the lights off or apply a
// profile, and what sleep and wake do. Words for the rail and the sheet.

export type Action =
  | { kind: 'nothing' }
  | { kind: 'off' }
  | { kind: 'back' }
  | { kind: 'profile'; name: string }

/** At `at` minutes after midnight on the `days` bits (bit 0 Monday .. bit 6 Sunday). */
export interface Rule {
  at: number
  days: number
  action: Action
}

export interface Schedule {
  rules: Rule[]
  sleep: Action
  wake: Action
}

export const EVERY_DAY = 0b111_1111
export const WEEKDAYS = 0b001_1111
export const WEEKEND = 0b110_0000
export const MAX_RULES = 32
/** Monday first, as the bits. */
export const DAY_NAMES = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday']
export const DAY_SHORT = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']

export const emptySchedule = (): Schedule => ({ rules: [], sleep: { kind: 'off' }, wake: { kind: 'back' } })

/** "07:05" for 425. */
export const clock = (at: number) =>
  `${String(Math.floor(at / 60)).padStart(2, '0')}:${String(at % 60).padStart(2, '0')}`

/** A time to read, in the system's clock (12- or 24-hour), as the time field shows it. */
export const timeText = (at: number, locale?: string) =>
  new Intl.DateTimeFormat(locale, { hour: '2-digit', minute: '2-digit', timeZone: 'UTC' }).format(
    new Date(Date.UTC(2000, 0, 1, Math.floor(at / 60), at % 60)),
  )

/** Minutes after midnight of "HH:MM", or null. */
export function minutes(s: string): number | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(s.trim())
  if (!m) return null
  const h = Number(m[1])
  const min = Number(m[2])
  return h < 24 && min < 60 ? h * 60 + min : null
}

/** "Every day", "Weekdays", "Weekends", "Mon, Wed, Fri" or "Tuesday". */
export function daysLabel(days: number): string {
  const d = days & EVERY_DAY
  if (d === EVERY_DAY) return 'Every day'
  if (d === WEEKDAYS) return 'Weekdays'
  if (d === WEEKEND) return 'Weekends'
  const on = DAY_SHORT.map((_, i) => i).filter((i) => d & (1 << i))
  if (on.length === 1) return DAY_NAMES[on[0]]
  return on.map((i) => DAY_SHORT[i]).join(', ')
}

export function actionLabel(a: Action): string {
  switch (a.kind) {
    case 'nothing':
      return 'Nothing'
    case 'off':
      return 'Lights off'
    case 'back':
      return 'Look from before'
    case 'profile':
      return a.name
  }
}

/** The rules in time order (the sheet and the rail list them so). */
export const byTime = (rules: Rule[]) => [...rules].sort((a, b) => a.at - b.at)

/** One line for the rail: "Sleep: lights off · Wake: look from before". */
export function powerLine(s: Schedule): string {
  const low = (a: Action) => (a.kind === 'profile' ? a.name : actionLabel(a).toLowerCase())
  return `Sleep: ${low(s.sleep)} · Wake: ${low(s.wake)}`
}

/** The next rule to fire after `now` (local time) within a week, with its moment. */
export function nextRule(s: Schedule, now: Date): { rule: Rule; at: Date } | null {
  let best: { rule: Rule; at: Date } | null = null
  for (const rule of s.rules) {
    for (let ahead = 0; ahead <= 7; ahead++) {
      const t = new Date(now.getFullYear(), now.getMonth(), now.getDate() + ahead, Math.floor(rule.at / 60), rule.at % 60)
      const bit = (t.getDay() + 6) % 7
      if (t > now && rule.days & (1 << bit)) {
        if (!best || t < best.at) best = { rule, at: t }
        break
      }
    }
  }
  return best
}

/** The action as a select value, and back: "off", "nothing", "back", "p:<name>". */
export const actionValue = (a: Action) => (a.kind === 'profile' ? `p:${a.name}` : a.kind)
export function actionOf(v: string): Action {
  if (v.startsWith('p:')) return { kind: 'profile', name: v.slice(2) }
  return { kind: v as 'nothing' | 'off' | 'back' }
}
