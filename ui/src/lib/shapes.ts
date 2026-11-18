// Which template each device is drawn with on the stage, chosen by the user. Remembered per
// device key (stable across OpenRGB restarts) in this window's storage; it is only a view setting.
import { isShape, type Shape } from './rig.ts'

const KEY = 'freergb-shapes'

export function loadShapes(): Record<string, Shape> {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(KEY) ?? '{}')
    if (!raw || typeof raw !== 'object') return {}
    return Object.fromEntries(Object.entries(raw).filter(([, v]) => isShape(v))) as Record<string, Shape>
  } catch {
    return {}
  }
}

export function saveShapes(m: Record<string, Shape>) {
  try {
    localStorage.setItem(KEY, JSON.stringify(m))
  } catch {
    /* private window or blocked storage: the choice lasts until the window closes */
  }
}
