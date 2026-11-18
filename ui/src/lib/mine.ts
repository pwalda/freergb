// "My effects" for every vial rack: the saved list and the actions the window owns (the sheet,
// the file dialogs), handed down by App through context instead of through every panel.
import { getContext, setContext } from 'svelte'
import type { SavedEffect } from './api.ts'
import type { EffectSpec } from './effects.ts'

export interface Mine {
  readonly saved: SavedEffect[]
  /** Open the sheet to keep this look under a name. */
  save: (effect: EffectSpec) => void
  /** Open the sheet for a saved effect (rename, replace with `current`, export, delete). */
  edit: (name: string, current?: EffectSpec) => void
  /** Pick a file and add it to My effects. */
  import: () => void
}

const KEY = Symbol('mine')
export const provideMine = (m: Mine) => setContext(KEY, m)
export const useMine = () => getContext<Mine | undefined>(KEY)
