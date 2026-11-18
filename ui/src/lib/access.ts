// What the window says about reaching OpenRGB and, on Linux, OpenRGB reaching the devices.

import { installUdevRules, rescanDevices, type ServerState } from './api.ts'

export const NO_SERVER_TEXT =
  'OpenRGB is not running, and freeRGB could not find it to start it. Install OpenRGB (on Linux, the “openrgb” package), then press Retry.'

/** A failure in plain words; the backend's own message where it already is. */
export const failText = (s: Extract<ServerState, { kind: 'failed' }>) => (s.reason === 'no_server' ? NO_SERVER_TEXT : s.message)

export const ACCESS_TEXT = 'OpenRGB may not have permission to reach your RGB devices.'
export const ACCESS_DETAIL = 'Setting up access installs OpenRGB’s device rules and asks for your system password once.'
export const ACCESS_DONE = 'Device access is set up. Looking for devices…'

/** Installs the rules, then has OpenRGB look again with its new access. Resolves with the
 *  error to show, or null. */
export async function setUpAccess(): Promise<string | null> {
  try {
    await installUdevRules()
  } catch (e) {
    return String(e)
  }
  try {
    await rescanDevices()
  } catch {
    // Not connected: OpenRGB picks the access up when it starts.
  }
  return null
}
