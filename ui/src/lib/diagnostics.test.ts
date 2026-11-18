import assert from 'node:assert/strict'
import { test } from 'node:test'
import { clockOf, hardwareText, LOG_CAP, logText, merge, pciId, shown, usbId, type LogLine, type LogView } from './diagnostics.ts'

const line = (seq: number, level = 3, text = `line ${seq}`): LogLine => ({ seq, level, time_ms: seq, file: 'x.cpp', line: 1, text })
const view = (epoch: number, lines: LogLine[]): LogView => ({ epoch, state: 'on', owned: true, lines })

test('the time reads from the start of OpenRGB', () => {
  assert.equal(clockOf(0), '0:00.000')
  assert.equal(clockOf(81_234), '1:21.234')
  assert.equal(clockOf(3_723_004), '1:02:03.004')
})

test('new lines add on, a new epoch starts over, and the oldest go past the cap', () => {
  const kept = [line(1), line(2)]
  assert.deepEqual(merge(kept, 4, view(4, [line(3)])).map((l) => l.seq), [1, 2, 3])
  assert.deepEqual(merge(kept, 4, view(5, [line(1)])).map((l) => l.seq), [1])
  const many = Array.from({ length: LOG_CAP }, (_, i) => line(i + 1))
  const next = merge(many, 1, view(1, [line(LOG_CAP + 1)]))
  assert.equal(next.length, LOG_CAP)
  assert.equal(next[0].seq, 2)
})

test('the filter keeps the chosen level and more serious, and dialogs always', () => {
  assert.equal(shown(line(1, 1), 2), true)
  assert.equal(shown(line(1, 3), 2), false)
  assert.equal(shown(line(1, 7), 2), true)
})

test('ids are written as the OS writes them', () => {
  assert.equal(usbId(0x1b1c, 0x0c1a), '1B1C:0C1A')
  const bus = { name: 'b', bus: 0, port: 0, pci_vendor: 0x8086, pci_device: 0x7a23, sub_vendor: 0, sub_device: 0 }
  assert.equal(pciId(bus), '8086:7A23')
  assert.equal(pciId({ ...bus, sub_vendor: 0x1462, sub_device: 0x7d25 }), '8086:7A23 (1462:7D25)')
  assert.equal(pciId({ ...bus, pci_vendor: -1, pci_device: -1 }), null)
})

test('copied text is plain and complete', () => {
  assert.equal(logText([line(81, 1, 'No reply')]), '[0:00.081] [Error] No reply')
  const text = hardwareText({
    buses: [],
    usb: [{ vendor: 0x046d, product: 0xc52b, manufacturer: 'Logitech', name: 'USB Receiver', serial: '' }],
    hid: [],
    serial: [{ port: 'COM3', vendor: 0x1a86, product: 0x7523, usb_path: '1-4' }],
  })
  assert.equal(
    text,
    'SMBus buses (0)\n\nUSB devices (1)\n  046D:C52B  Logitech USB Receiver\n\nHID devices (0)\n\nSerial ports (1)\n  COM3  USB 1A86:7523  1-4',
  )
})
