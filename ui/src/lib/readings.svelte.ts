// What a look can follow on this machine and the readings now, for the controls. The preview
// keeps its own copy (preview.ts `setReadings`), since it also runs outside Svelte in tests.

import type { SourceId } from './effects'
import type { Readings } from './react'

export const sensing = $state<{ sources: SourceId[]; needs: Partial<Record<SourceId, string>>; now: Readings }>({
  sources: [],
  needs: {},
  now: {},
})
