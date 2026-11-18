import { colorSlots, DEFAULT_PARAMS, OWN_PALETTE, type EffectKind, type SceneParams } from './effects.ts'

/** The settings for choosing `kind` from the Looks row: the running look's colours, speed,
 *  brightness and so on carry over, so switching effect doesn't throw the tuning away. A look with a palette of its own
 *  (fire) starts from that palette instead. */
export function lookParams(kind: EffectKind, from?: SceneParams): SceneParams {
  // Lights off is an all-black look: nothing worth carrying into the next one.
  const dark = from && from.colors.slice(0, Math.max(1, colorSlots(from))).every((c) => c.toLowerCase() === '#000000')
  const base = from && !dark ? from : DEFAULT_PARAMS
  // A look with its own palette keeps the running colours only when it is already running.
  const own = from?.kind === kind ? undefined : OWN_PALETTE[kind]
  // A direction belongs to its look: rain picked after a sideways wave still falls.
  const { angle, ...rest } = base
  return { ...rest, colors: [...(own ?? base.colors)], kind, ...(from?.kind === kind && angle != null ? { angle } : {}) }
}
