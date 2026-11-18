import type { IconName } from './icons.ts'

// OpenRGB device type ids (crates/freergb-protocol/src/ids.rs, `device_type`).
const KINDS: Record<number, [string, IconName]> = {
  0: ['Motherboard', 'board'],
  1: ['Memory', 'ram'],
  2: ['Graphics card', 'gpu'],
  3: ['Cooler', 'fan'],
  4: ['LED strip', 'strip'],
  5: ['Keyboard', 'keyboard'],
  6: ['Mouse', 'mouse'],
  7: ['Mousepad', 'mouse'],
  8: ['Headset', 'headset'],
  9: ['Headset stand', 'headset'],
  10: ['Gamepad', 'chip'],
  11: ['Light', 'strip'],
  12: ['Speaker', 'speaker'],
  13: ['Virtual device', 'chip'],
  14: ['Storage', 'storage'],
  15: ['Case', 'case'],
  16: ['Microphone', 'chip'],
  17: ['Accessory', 'chip'],
  18: ['Keypad', 'keyboard'],
  19: ['Laptop', 'monitor'],
  20: ['Monitor', 'monitor'],
}

export function deviceKind(type: number): { label: string; icon: IconName } {
  const [label, icon] = KINDS[type] ?? ['Device', 'chip']
  return { label, icon }
}
