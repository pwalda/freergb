// The desk map as the window uses it: each piece's drawing (shape, a strip's length) and its
// spot, read from and written back to the backend's map entries (`Placed`, see api.ts). A piece
// is a device, or one of its segments (each segment sits on the map by itself).

import type { Device, Piece, Placed } from './api.ts'
import { entryOfPiece, piecesOf } from './preview.ts'
import { defaultShape, isShape, ledSpots, placementOf, spotOf, template, type Shape, type Spot, type Template } from './rig.ts'
import { keyId } from './scenes.ts'

/** One piece while arranging. */
export interface Draft {
  shape: Shape
  /** A strip's length in dots; null draws a long strip as rows. */
  length: number | null
  spot: Spot | null
}

/** One piece on the stage: which device (index) and which of its LEDs. */
export interface Part {
  dev: number
  piece: Piece
}

/** Every device's pieces, in device order. */
export const partsOf = (devices: Device[]): Part[] => devices.flatMap((d, dev) => piecesOf(d).map((piece) => ({ dev, piece })))

/** A piece's id, stable while its device and segment stay. */
export const partId = (d: Device, p: Piece) => (p.segment ? `${keyId(d.key)}|${p.segment.zone}:${p.segment.start}` : keyId(d.key))

/** What the stage calls a piece: the device's name, or the segment's after it. */
export const partName = (deviceName: string, p: Piece) => (p.segment ? `${deviceName} · ${p.name}` : deviceName)

/** How a piece is drawn: its map entry, else (the device itself) an older per-window choice
 *  or its type; a segment is a strip. */
export function draftOf(map: Placed[], d: Device, p: Piece, legacy: Record<string, Shape> = {}): Draft {
  const e = entryOfPiece(map, d, p.segment)
  const saved = e?.shape
  const shape = isShape(saved) ? saved : p.segment ? 'strip' : (legacy[keyId(d.key)] ?? defaultShape(d.device_type))
  const length = shape === 'strip' && e?.length ? e.length : null
  const t = templateOf(p.leds.length, shape, length)
  return { shape, length, spot: e?.at ? spotOf(t, e.at) : null }
}

export const templateOf = (leds: number, shape: Shape, length: number | null): Template =>
  template(shape, leds, shape === 'strip' ? (length ?? undefined) : undefined)

/** A strip's length when arranging starts: its LEDs, up to the 30 dots of one row. */
export const startLength = (leds: number) => Math.max(1, Math.min(leds, 30))

/** The map entry for one piece. A keyboard with a matrix keeps the engine's key grid (its real
 *  layout); every other piece sends the LED spots of its drawing. */
export function entryFor(d: Device, p: Piece, draft: Draft): Placed {
  const t = templateOf(p.leds.length, draft.shape, draft.length)
  const matrix = !p.segment && d.zones.some((z) => z.matrix)
  const e: Placed = { device: d.key, shape: draft.shape }
  if (p.segment) e.segment = p.segment
  if (draft.shape === 'strip' && draft.length) e.length = draft.length
  if (draft.spot) e.at = placementOf(t, draft.spot)
  if (!matrix && p.leds.length > 0) e.points = ledSpots(t, p.leds.length)
  return e
}

/** The new map: the entries of these pieces, and those of devices not connected now kept. */
export function withEntries(map: Placed[], devices: Device[], parts: Part[], drafts: Draft[]): Placed[] {
  const absent = map.filter((p) => !devices.some((d) => keyId(d.key) === keyId(p.device)))
  return [...parts.map((p, i) => entryFor(devices[p.dev], p.piece, drafts[i])), ...absent]
}
