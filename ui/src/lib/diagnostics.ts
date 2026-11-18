// What OpenRGB reports for checking a problem: its log and the hardware it sees.

export type LogState = 'unavailable' | 'off' | 'waiting' | 'on'

export interface LogLine {
  seq: number
  /** OpenRGB's level: 0 fatal, 1 error, 2 warning, 3 info, 4 verbose, 5 debug, 6 trace, 7 dialog. */
  level: number
  /** Since OpenRGB started. */
  time_ms: number
  file: string
  line: number
  text: string
}

export interface LogView {
  epoch: number
  state: LogState
  /** freeRGB started this OpenRGB, so turning the log on restarts it. */
  owned: boolean
  lines: LogLine[]
}

export interface Bus {
  name: string
  bus: number
  port: number
  pci_vendor: number
  pci_device: number
  sub_vendor: number
  sub_device: number
}
export interface Usb {
  vendor: number
  product: number
  manufacturer: string
  name: string
  serial: string
}
export interface Hid extends Usb {
  usage_page: number
  usage: number
  interface: number
  path: string
}
export interface Serial {
  port: string
  vendor: number | null
  product: number | null
  usb_path: string | null
}
export interface Hardware {
  buses: Bus[]
  usb: Usb[]
  hid: Hid[]
  serial: Serial[]
}

/** Lines the window keeps, like the backend. */
export const LOG_CAP = 5000

const LEVELS = ['Fatal', 'Error', 'Warning', 'Info', 'Verbose', 'Debug', 'Trace', 'Dialog']
export const levelName = (level: number) => LEVELS[level] ?? `Level ${level}`

/** How much of the log to show: each choice shows its level and everything more serious. */
export const SHOW = [
  { value: 2, label: 'Problems' },
  { value: 3, label: 'Usual' },
  { value: 6, label: 'Everything' },
] as const

/** Dialog lines (7) are messages OpenRGB showed the user: always worth seeing. */
export const shown = (l: LogLine, upTo: number) => l.level <= upTo || l.level === 7

/** Problems get a status mark: errors (and fatal) and warnings. */
export const severity = (level: number): 'bad' | 'warn' | null =>
  level <= 1 ? 'bad' : level === 2 ? 'warn' : null

/** `m:ss.mmm` since OpenRGB started; hours when it has run that long. */
export function clockOf(ms: number): string {
  const s = Math.floor(ms / 1000)
  const pad = (n: number, w = 2) => String(n).padStart(w, '0')
  const tail = `${pad(s % 60)}.${pad(ms % 1000, 3)}`
  const m = Math.floor(s / 60)
  return m >= 60 ? `${Math.floor(m / 60)}:${pad(m % 60)}:${tail}` : `${m}:${tail}`
}

/** Add new lines to the kept ones (or start over on a new epoch), keeping the newest `LOG_CAP`. */
export function merge(kept: LogLine[], keptEpoch: number | null, view: LogView): LogLine[] {
  const all = view.epoch === keptEpoch ? kept.concat(view.lines) : view.lines
  return all.length > LOG_CAP ? all.slice(all.length - LOG_CAP) : all
}

/** A USB id as OpenRGB and the OS write it: four upper-case hex digits. */
export const hex4 = (n: number) => (n & 0xffff).toString(16).toUpperCase().padStart(4, '0')
export const usbId = (vendor: number, product: number) => `${hex4(vendor)}:${hex4(product)}`

/** A bus's PCI ids, or null when it has none (OpenRGB sends 0 or -1). */
export function pciId(b: Bus): string | null {
  if (b.pci_vendor <= 0 && b.pci_device <= 0) return null
  const sub = b.sub_vendor > 0 || b.sub_device > 0 ? ` (${usbId(b.sub_vendor, b.sub_device)})` : ''
  return `${usbId(b.pci_vendor, b.pci_device)}${sub}`
}

/** The device's own words, or "Unnamed". */
export const nameOf = (d: Usb) => [d.manufacturer, d.name].filter(Boolean).join(' ') || 'Unnamed'

export const logText = (lines: LogLine[]) =>
  lines.map((l) => `[${clockOf(l.time_ms)}] [${levelName(l.level)}] ${l.text}`).join('\n')

export function hardwareText(h: Hardware): string {
  const part = (title: string, rows: string[]) => [`${title} (${rows.length})`, ...rows.map((r) => `  ${r}`)].join('\n')
  return [
    part(
      'SMBus buses',
      h.buses.map((b) => [b.name, pciId(b) && `PCI ${pciId(b)}`].filter(Boolean).join('  ')),
    ),
    part(
      'USB devices',
      h.usb.map((d) => [usbId(d.vendor, d.product), nameOf(d), d.serial && `serial ${d.serial}`].filter(Boolean).join('  ')),
    ),
    part(
      'HID devices',
      h.hid.map((d) =>
        [usbId(d.vendor, d.product), nameOf(d), `usage ${hex4(d.usage_page)}:${hex4(d.usage)}`, `interface ${d.interface}`, d.path]
          .filter(Boolean)
          .join('  '),
      ),
    ),
    part(
      'Serial ports',
      h.serial.map((s) =>
        [s.port, s.vendor !== null && s.product !== null && `USB ${usbId(s.vendor, s.product)}`, s.usb_path]
          .filter(Boolean)
          .join('  '),
      ),
    ),
  ].join('\n\n')
}
