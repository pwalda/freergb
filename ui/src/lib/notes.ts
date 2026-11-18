// The user's names for devices and the devices freeRGB leaves alone (kept by the backend, see
// `DeviceNote` in store.rs).

import type { Device, DeviceNote } from './api.ts'
import { sameKey } from './scenes.ts'

export const noteOf = (notes: DeviceNote[], d: Device) => notes.find((n) => sameKey(n.device, d.key))

export const isHidden = (notes: DeviceNote[], d: Device) => noteOf(notes, d)?.hidden === true

/** The devices the window shows and effects drive. */
export const shownDevices = (devices: Device[], notes: DeviceNote[]) => devices.filter((d) => !isHidden(notes, d))

/** Each device's name: the user's, else its own, numbered when identical devices share it. */
export function displayNames(devices: Device[], notes: DeviceNote[]): string[] {
  const own = devices.map((d) => noteOf(notes, d)?.name ?? null)
  const count = new Map<string, number>()
  devices.forEach((d, i) => {
    if (own[i] === null) count.set(d.name, (count.get(d.name) ?? 0) + 1)
  })
  return devices.map((d, i) => own[i] ?? ((count.get(d.name) ?? 0) > 1 ? `${d.name} #${d.key.nth + 1}` : d.name))
}
