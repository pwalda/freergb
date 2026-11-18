// Writes dist/licenses.txt at build time: the licence of every npm package that ends up in
// the bundle, then the Rust crates' (generated/rust-licenses.txt, made by cargo-about in
// tools/ci/licenses.sh). The About sheet shows it. Fonts are in THIRD-PARTY-NOTICES.md.

import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import type { Plugin } from 'vite'

const RULE = '='.repeat(80)

/** `/x/node_modules/@scope/pkg/dist/a.js` → `/x/node_modules/@scope/pkg`. */
export function packageRoot(id: string): string | null {
  const at = id.lastIndexOf('/node_modules/')
  if (at < 0) return null
  const rest = id.slice(at + '/node_modules/'.length).split('/')
  const n = rest[0].startsWith('@') ? 2 : 1
  return id.slice(0, at) + '/node_modules/' + rest.slice(0, n).join('/')
}

function licenceText(root: string): string {
  const file = readdirSync(root).find((f) => /^(licen[cs]e|copying)(\.|-|$)/i.test(f))
  return file ? readFileSync(join(root, file), 'utf8').trim() : '(No licence file in the package.)'
}

export function licenses(): Plugin {
  return {
    name: 'freergb-licenses',
    apply: 'build',
    generateBundle(_, bundle) {
      const roots = new Set<string>()
      for (const out of Object.values(bundle))
        if (out.type === 'chunk')
          for (const id of Object.keys(out.modules)) {
            const r = packageRoot(id.replace(/\\/g, '/'))
            if (r) roots.add(r)
          }
      const js = [...roots]
        .map((r) => ({ r, p: JSON.parse(readFileSync(join(r, 'package.json'), 'utf8')) }))
        .sort((a, b) => a.p.name.localeCompare(b.p.name))
        .map(({ r, p }) => `${RULE}\n${p.name} ${p.version} (${p.license ?? 'see below'})\n\n${licenceText(r)}\n`)
      const rust = join(import.meta.dirname, 'generated', 'rust-licenses.txt')
      const parts = [
        'freeRGB is built with these open-source libraries. Thank you to their authors.\n',
        'JavaScript packages in the window\n',
        ...js,
        '\nRust crates in the app and the sensor service\n',
        existsSync(rust) ? readFileSync(rust, 'utf8') : '(Listed in release builds.)\n',
      ]
      this.emitFile({ type: 'asset', fileName: 'licenses.txt', source: parts.join('\n') })
    },
  }
}
