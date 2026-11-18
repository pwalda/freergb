import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import { failText, NO_SERVER_TEXT } from './access.ts'

test('a missing OpenRGB is said in plain words; other failures keep their message', () => {
  assert.equal(failText({ kind: 'failed', reason: 'no_server', message: 'No OpenRGB server is running on port 6742' }), NO_SERVER_TEXT)
  assert.equal(failText({ kind: 'failed', reason: 'port_in_use', message: 'Port 6742 is taken.' }), 'Port 6742 is taken.')
})
