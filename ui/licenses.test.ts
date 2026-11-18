import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import { packageRoot } from './licenses.ts'

test('a module belongs to the package it sits in, scoped or not', () => {
  assert.equal(packageRoot('/w/ui/node_modules/svelte/src/internal/client/index.js'), '/w/ui/node_modules/svelte')
  assert.equal(packageRoot('/w/ui/node_modules/@tauri-apps/api/core.js'), '/w/ui/node_modules/@tauri-apps/api')
  assert.equal(packageRoot('/w/ui/node_modules/a/node_modules/b/x.js'), '/w/ui/node_modules/a/node_modules/b')
  assert.equal(packageRoot('/w/ui/src/App.svelte'), null)
})
