// Pure helpers for the scene list. The backend list is the truth: a device's own effect is
// the last scene that targets exactly that device, and `all` scenes are the fallback under it
// (the engine lets a later scene win a device).

import type { DeviceKey, EffectSpec, Scene } from './effects.ts'

export const sameKey = (a: DeviceKey, b: DeviceKey) =>
  a.name === b.name && a.location === b.location && a.serial === b.serial && a.nth === b.nth

/** A stable string for `{#each}` keys and maps; unlike the controller id it survives a restart. */
export const keyId = (k: DeviceKey) => JSON.stringify([k.name, k.location, k.serial, k.nth])

const onlyTargets = (s: Scene, key: DeviceKey) =>
  !s.group && s.targets !== 'all' && s.targets.devices.length === 1 && sameKey(s.targets.devices[0], key)

/** A named set of devices sharing one canvas (mirrors `Group` in the app's store). */
export interface Group {
  name: string
  devices: DeviceKey[]
}

/** The group this device belongs to, if any (a device is in one group at most). */
export const groupOf = (groups: Group[], key: DeviceKey) => groups.find((g) => g.devices.some((k) => sameKey(k, key)))

/** The look of group `name`, if it has one. */
export function groupScene(scenes: Scene[], name: string): Scene | undefined {
  return scenes.findLast((s) => s.group === name)
}

/** The look a device shows when it has none of its own: its group's, else the all-devices one. */
export function inheritedScene(scenes: Scene[], groups: Group[], key: DeviceKey): Scene | undefined {
  const g = groupOf(groups, key)
  return (g && groupScene(scenes, g.name)) ?? globalScene(scenes)
}

/** `scenes` with group `name`'s look replaced (or removed, for `null`). Group looks sit after
 *  the all-devices look and before devices' own looks, so a device's own look still wins. */
export function withGroupScene(scenes: Scene[], name: string, scene: Scene | null): Scene[] {
  const all = scenes.filter((s) => s.targets === 'all')
  const groupLooks = scenes.filter((s) => s.targets !== 'all' && s.group && s.group !== name)
  const rest = scenes.filter((s) => s.targets !== 'all' && !s.group)
  return [...all, ...groupLooks, ...(scene ? [scene] : []), ...rest]
}

/** How many groups have a look of their own. */
export const groupLookCount = (scenes: Scene[]) => new Set(scenes.flatMap((s) => (s.group ? [s.group] : []))).size

/** The effect set on this device alone. */
export function deviceScene(scenes: Scene[], key: DeviceKey): Scene | undefined {
  return scenes.findLast((s) => onlyTargets(s, key))
}

/** The effect running on all devices, which a device follows unless it has its own. */
export function globalScene(scenes: Scene[]): Scene | undefined {
  return scenes.findLast((s) => s.targets === 'all')
}

/** `scenes` with this device's own effect replaced (or removed, for `null`). Other devices'
 *  scenes, including ones that name several devices, are left as they were. */
export function withDeviceScene(scenes: Scene[], key: DeviceKey, scene: Scene | null): Scene[] {
  const rest = scenes.filter((s) => !onlyTargets(s, key))
  return scene ? [...rest, scene] : rest
}

/** `scenes` with the all-devices effect replaced (or removed, for `null`). It goes first so
 *  every per-device effect keeps winning its device, since a later scene wins. */
export function withGlobalScene(scenes: Scene[], scene: Scene | null): Scene[] {
  const rest = scenes.filter((s) => s.targets !== 'all')
  return scene ? [scene, ...rest] : rest
}

/** How many devices have an effect of their own on top of the `all` scene. */
export function ownEffectCount(scenes: Scene[]): number {
  const keys = new Set<string>()
  for (const s of scenes) {
    if (!s.group && s.targets !== 'all' && s.targets.devices.length === 1) keys.add(keyId(s.targets.devices[0]))
  }
  return keys.size
}

const canon = (v: unknown): unknown =>
  Array.isArray(v)
    ? v.map(canon)
    : v && typeof v === 'object'
      ? Object.fromEntries(
          Object.entries(v)
            .sort(([a], [b]) => a.localeCompare(b))
            .map(([k, x]) => [k, canon(x)]),
        )
      : v

/** Two effects are the same look (property order ignored). */
export const sameSpec = (a: EffectSpec | undefined, b: EffectSpec | undefined) =>
  JSON.stringify(canon(a ?? null)) === JSON.stringify(canon(b ?? null))

/** Every device black: what Lights off (window or tray) sets. */
export const OFF_SCENE: Scene = {
  effect: { type: 'static', colors: ['#000000'], speed: 0, density: 1, brightness: 1, direction: 'forward' },
  targets: 'all',
  canvas: 'shared',
}

/** The lights are off: one look on every device, and it is black (or at zero brightness). */
export const lightsOut = (scenes: Scene[]) =>
  scenes.length === 1 &&
  scenes[0].targets === 'all' &&
  !scenes[0].group &&
  scenes[0].effect.type === 'static' &&
  ((scenes[0].effect.brightness ?? 1) === 0 || (scenes[0].effect.colors ?? []).every((c) => c.toLowerCase() === '#000000'))

/** Equality that ignores property order: Rust serialises fields in its own order. */
export const sameScene = (a: Scene | undefined, b: Scene | undefined) =>
  JSON.stringify(canon(a ?? null)) === JSON.stringify(canon(b ?? null))
