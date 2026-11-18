// What the window says about OpenRGB looking for devices (a rescan or its own detection).

import type { Detection, Device } from './api.ts'
import { sameKey } from './scenes.ts'

export const SCAN_TEXT = 'Looking for devices…'

/** The line under the rail's progress bar: how far, and the detector running. */
export const scanDetail = (d: Detection) => [d.percent === null ? '' : `${d.percent}%`, d.label].filter(Boolean).join(' · ')

export const scanShort = (d: Detection) => (d.percent === null ? 'Scanning…' : `Scanning ${d.percent}%`)

const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`

/** The toast after a scan this window asked for: what came and went. */
export function scanOutcome(before: Device[], after: Device[]): string {
  const found = after.filter((d) => !before.some((b) => sameKey(b.key, d.key)))
  const gone = before.filter((b) => !after.some((d) => sameKey(b.key, d.key)))
  const parts: string[] = []
  if (found.length === 1) parts.push(`Found a new device: ${found[0].name}`)
  else if (found.length > 1) parts.push(`Found ${count(found.length, 'new device', 'new devices')}`)
  if (gone.length > 0) parts.push(`${count(gone.length, 'device is', 'devices are')} gone`)
  return parts.length > 0 ? `${parts.join('; ')}.` : 'Scan finished. No new devices.'
}
