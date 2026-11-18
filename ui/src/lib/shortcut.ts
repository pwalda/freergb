/** Number keys pick looks (1-6, 0 = follow all devices). Ignored while typing in a field, with a
 *  modifier held, or while a dialog (the painter) is open. */
export function shortcut(e: KeyboardEvent, onDigit: (n: number) => unknown) {
  if (e.ctrlKey || e.metaKey || e.altKey || e.repeat) return
  if (!/^[0-9]$/.test(e.key)) return
  const t = e.target
  if (t instanceof HTMLElement && t.closest('input, select, textarea, [contenteditable], dialog')) return
  if (document.querySelector('dialog[open]')) return
  onDigit(Number(e.key))
}
