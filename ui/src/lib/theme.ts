export type Theme = 'dark' | 'light'

/** The user's stored choice wins; otherwise follow the system. Dark when the system has no preference. */
export function resolveTheme(stored: string | null, systemPrefersLight: boolean): Theme {
  if (stored === 'dark' || stored === 'light') return stored
  return systemPrefersLight ? 'light' : 'dark'
}
