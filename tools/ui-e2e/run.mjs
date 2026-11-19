// Drives the built UI (ui/dist) in headless Chromium against a fake Tauri backend.
// Setup and usage: see README.md. Not part of CI (it needs a browser download).
import { chromium } from 'playwright-core'
import http from 'node:http'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const DIST = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../ui/dist')
// Optional: a specific Chromium binary; by default playwright-core's own download is used.
const EXE = process.env.CHROMIUM_EXE || undefined
const SHOTS = process.argv[2]
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.woff2': 'font/woff2' }
const server = http.createServer((req, res) => {
  const f = path.join(DIST, req.url === '/' ? 'index.html' : req.url.split('?')[0])
  if (!f.startsWith(DIST) || !fs.existsSync(f)) { res.writeHead(404); return res.end() }
  res.writeHead(200, { 'content-type': mime[path.extname(f)] ?? 'application/octet-stream' })
  res.end(fs.readFileSync(f))
}).listen(0)
const url = `http://127.0.0.1:${server.address().port}/`

const fakeTauri = () => {
  const key = (nth) => ({ name: 'Vengeance RGB', location: 'I2C 0x58', serial: '', nth })
  const dev = (id, nth) => ({
    id, key: key(nth), name: 'Vengeance RGB', vendor: 'Corsair', device_type: 1, active_mode: 1,
    description: 'Corsair Vengeance RGB DRAM', version: '1.4', serial: '', location: `I2C: /dev/i2c-0, address 0x5${8 + nth}`,
    modes: [
      { index: 1, name: 'Static', speed: null, brightness: 1, directions: [], direction: null, colors_min: 1, colors_max: 1, colors: ['#ff0000'], saveable: false },
      { index: 3, name: 'Breathing', speed: 0.5, brightness: 1, directions: ['left', 'right', 'up', 'down'], direction: 'up', colors_min: 2, colors_max: 3, colors: ['#ff0000', '#0000ff'], saveable: true },
      { index: 4, name: 'Off', speed: null, brightness: null, directions: [], direction: null, colors_min: 0, colors_max: 0, colors: [], saveable: false },
    ], zones: [nth === 0 ? { index: 0, name: 'Strip', leds: 60, min_leds: 1, max_leds: 300, resizable: true, start: 0, matrix: null } : { index: 0, name: 'RAM', leds: 8, min_leds: 8, max_leds: 8, resizable: false, start: 0, matrix: null }],
    colors: Array.from({ length: 8 }, (_, i) => `#${(i * 30).toString(16).padStart(2, '0')}2060`),
  })
  const w = window
  w.__calls = []
  w.__scenes = []
  w.__devices = sessionStorage.getItem('nodevices') ? [] : [dev(1, 0), dev(2, 1)]
  const listeners = {}
  let cb = 1
  // Profiles as the backend keeps them: the active one is the one whose look is running.
  w.__profiles = []
  const same = (a, b) => JSON.stringify(a) === JSON.stringify(b)
  const profileView = () => ({ names: w.__profiles.map((p) => p.name), active: w.__profiles.find((p) => same(p.scenes, w.__scenes))?.name ?? null })
  const emitProfiles = () => setTimeout(() => w.__emit('profiles', profileView()), 0)
  const findProfile = (name) => { const p = w.__profiles.find((x) => x.name === name); if (!p) throw `No profile called “${name}”`; return p }
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} }
  // Fields only the per-zone-modes check sets: every other fake device has none.
  const fill = (ds) => ds.forEach((d) => {
    d.modes.forEach((m) => { m.whole_device ??= false })
    d.zones.forEach((z) => { z.modes ??= []; z.active_mode ??= -1 })
  })
  w.__emit = (event, payload) => {
    if (event === 'devices') fill(payload)
    ;(listeners[event] ?? []).forEach((id) => w[`_${id}`]({ event, id, payload }))
  }
  w.__TAURI_INTERNALS__ = {
    transformCallback(f) { const id = cb++; w[`_${id}`] = f; return id },
    unregisterCallback() {},
    convertFileSrc: (p) => p,
    metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main', windowLabel: 'main' } },
    async invoke(cmd, args) {
      if (cmd === 'plugin:event|listen') { (listeners[args.event] ??= []).push(args.handler); return args.handler }
      if (cmd === 'plugin:event|unlisten') return
      // Windows: no system title bar (sessionStorage 'frame'), so the UI draws one.
      if (cmd === 'plugin:window|is_decorated') return sessionStorage.getItem('frame') !== '1'
      if (cmd === 'plugin:window|is_maximized') return !!w.__maxed
      if (cmd === 'plugin:window|toggle_maximize') { w.__maxed = !w.__maxed; return }
      if (cmd === 'plugin:window|minimize' || cmd === 'plugin:window|close') { (w.__winCalls ??= []).push(cmd.split('|')[1]); return }
      if (cmd === 'server_state') return { kind: 'connected', owned: false, server_name: 'OpenRGB (fake)' }
      if (cmd === 'devices') { fill(w.__devices); return w.__devices }
      if (cmd === 'scenes') return w.__scenes
      if (cmd === 'retry_server') return
      if (cmd === 'profiles') return profileView()
      if (cmd === 'store_notice') return null
      if (cmd === 'autostart') return !!w.__autostart
      if (cmd === 'set_autostart') { w.__autostart = args.on; return args.on }
      if (cmd === 'reset_all') { sessionStorage.setItem('reset', '1'); return }
      if (cmd === 'fps') return w.__fps ?? 30
      if (cmd === 'update_status') return w.__update ?? null
      if (cmd === 'about') return { version: '0.1.0-beta.1', bundled_openrgb: '1.1' }
      if (cmd === 'take_about_request') { const r = !!w.__aboutAsked; w.__aboutAsked = false; return r }
      if (cmd === 'open_link') { (w.__links ??= []).push(args.id); return }
      // Linux without OpenRGB's udev rules, only when a check asks for it; elsewhere nothing to set up.
      if (cmd === 'udev_rules') return sessionStorage.getItem('udev') === 'missing' ? false : null
      if (cmd === 'install_udev_rules') {
        w.__udevInstalls = (w.__udevInstalls ?? 0) + 1
        if (w.__udevInstalls === 1) throw 'Device access was not set up: the password prompt was cancelled.'
        sessionStorage.removeItem('udev')
        return
      }
      if (cmd === 'install_update') {
        // Part of the download, then the network drops: the real one restarts on success.
        setTimeout(() => w.__emit('update-progress', 40), 0)
        await new Promise((r) => setTimeout(r, 150))
        throw 'The update could not be downloaded: connection reset'
      }
      if (cmd === 'schedule') return w.__schedule ?? { rules: [], sleep: { kind: 'off' }, wake: { kind: 'back' } }
      if (cmd === 'set_schedule') {
        if (args.schedule.rules.some((r) => (r.days & 127) === 0)) throw 'A time needs at least one day'
        w.__schedule = args.schedule
        setTimeout(() => w.__emit('schedule', args.schedule), 0)
        return
      }
      if (cmd === 'server_log') {
        // The real command takes an unsigned epoch or null; anything else fails there.
        if (args.epoch !== null && !(Number.isInteger(args.epoch) && args.epoch >= 0)) throw new Error(`invalid args \`epoch\`: ${args.epoch}`)
        const lg = (w.__log ??= { epoch: 1, state: 'off', owned: true, lines: [] })
        return { epoch: lg.epoch, state: lg.state, owned: lg.owned, lines: args.epoch === lg.epoch ? lg.lines.filter((l) => l.seq > args.after) : lg.lines }
      }
      if (cmd === 'turn_on_server_log') {
        const lg = w.__log
        // An owned OpenRGB restarts: the log comes back on, with its first lines.
        setTimeout(() => {
          if (lg.owned) {
            lg.state = 'on'
            lg.epoch++
            lg.lines = [
              { seq: 1, level: 3, time_ms: 12, file: 'NetworkServer.cpp', line: 1236, text: '[NetworkServer] Network server started' },
              { seq: 2, level: 5, time_ms: 40, file: 'i2c_smbus.cpp', line: 80, text: '[i2c_smbus] Scanning bus 0' },
              { seq: 3, level: 2, time_ms: 1834, file: 'Detector.cpp', line: 77, text: '[CorsairPeripheral] No reply from device on /dev/hidraw3' },
              { seq: 4, level: 1, time_ms: 2210, file: 'Detector.cpp', line: 91, text: '[ENE SMBus] Failed to read register 0x80' },
            ]
          } else lg.state = 'waiting'
          w.__emit('server-log', null)
        }, 30)
        return
      }
      if (cmd === 'clear_server_log') {
        w.__log.lines = []
        w.__log.epoch++
        setTimeout(() => w.__emit('server-log', null), 0)
        return
      }
      if (cmd === 'hardware') return {
        buses: [{ name: 'SMBus I801 adapter at efa0', bus: 0, port: 0, pci_vendor: 0x8086, pci_device: 0x7a23, sub_vendor: 0x1462, sub_device: 0x7d25 }],
        usb: [{ vendor: 0x1b1c, product: 0x0c1a, manufacturer: 'Corsair', name: 'iCUE H150i ELITE', serial: '' }],
        hid: [{ vendor: 0x046d, product: 0xc52b, manufacturer: 'Logitech', name: 'USB Receiver', serial: '', usage_page: 1, usage: 6, interface: 0, path: '/dev/hidraw0' }],
        serial: [{ port: '/dev/ttyUSB0', vendor: 0x1a86, product: 0x7523, usb_path: '1-4' }],
      }
      if (cmd === 'detection') return w.__detection ?? { scanning: false, percent: null, label: '' }
      if (cmd === 'rescan_devices') {
        w.__rescans = (w.__rescans ?? 0) + 1
        const at = (ms, d) => setTimeout(() => { w.__detection = d; w.__emit('detection', d) }, ms)
        at(0, { scanning: true, percent: null, label: '' })
        at(300, { scanning: true, percent: 40, label: 'Corsair Vengeance' })
        if (!w.__holdScan) {
          setTimeout(() => {
            w.__devices = [...w.__devices, { ...dev(9, 0), id: 9, key: { name: 'Lighting Node', location: 'HID 9', serial: '', nth: 0 }, name: 'Lighting Node' }]
            w.__emit('devices', w.__devices)
          }, 900)
          at(1000, { scanning: false, percent: null, label: '' })
        }
        return null
      }
      if (cmd === 'device_notes') return w.__notes ?? []
      if (cmd === 'set_device_note') {
        const same = (a, b) => a.name === b.name && a.location === b.location && a.serial === b.serial && a.nth === b.nth
        let list = (w.__notes ?? []).filter((n) => !same(n.device, args.device))
        const name = args.name?.trim() || null
        if (name || args.hidden) list = [...list, { device: args.device, ...(name ? { name } : {}), hidden: args.hidden }]
        w.__notes = list
        ;(w.__noteCalls ??= []).push(args)
        setTimeout(() => w.__emit('device-notes', list), 0)
        return null
      }
      if (cmd === 'sources') return w.__sources ?? [{ id: 'cpu_temp', needs: 'Needs LibreHardwareMonitor running' }, { id: 'sound' }, { id: 'screen' }]
      if (cmd === 'readings') return w.__readings ?? {}
      if (cmd === 'desk_map') return w.__map ?? []
      if (cmd === 'set_desk_map') { w.__map = args.map; w.__mapCalls = (w.__mapCalls ?? 0) + 1; return null }
      if (cmd === 'set_fps') { w.__fps = args.fps; return args.fps }
      // Groups as the backend keeps them: a group's look follows its name and devices.
      if (cmd === 'groups') return w.__groups ?? []
      // My effects, and the file dialogs as the backend would answer them.
      if (cmd === 'my_effects') return w.__saved ?? []
      if (cmd === 'save_effect' || cmd === 'delete_effect' || cmd === 'import_effect') {
        let list = w.__saved ?? []
        let out
        if (cmd === 'save_effect') {
          out = { name: args.name.trim(), effect: args.effect }
          const at = list.findIndex((x) => x.name === (args.old ?? out.name))
          list = at >= 0 ? list.map((x, i) => (i === at ? out : x)) : [...list, out]
        } else if (cmd === 'delete_effect') list = list.filter((x) => x.name !== args.name)
        else {
          out = w.__importNext ?? null
          if (out) list = [...list, out]
        }
        w.__saved = list
        setTimeout(() => w.__emit('effects', list), 0)
        return out
      }
      if (cmd === 'export_effect') { ;(w.__exported ??= []).push(args.name); return w.__exportTo ?? null }
      if (cmd === 'save_group' || cmd === 'delete_group') {
        const groups = w.__groups ?? []
        let g = null
        let from = args.name
        if (cmd === 'save_group') {
          g = { name: args.name.trim(), devices: args.devices }
          if (g.devices.length < 2) throw 'A group needs at least two devices'
          from = args.old ?? g.name
          const at = groups.findIndex((x) => x.name === args.old)
          w.__groups = at >= 0 ? groups.map((x, i) => (i === at ? g : x)) : [...groups, g]
        } else w.__groups = groups.filter((x) => x.name !== args.name)
        w.__scenes = w.__scenes.flatMap((sc) => (sc.group !== from ? [sc] : g ? [{ ...sc, group: g.name, targets: { devices: g.devices } }] : []))
        ;(w.__groupCalls ??= []).push([cmd, JSON.parse(JSON.stringify(args))])
        const sent = w.__scenes
        setTimeout(() => { w.__emit('scenes', sent); w.__emit('groups', w.__groups) }, 0)
        return g ?? undefined
      }
      if (cmd === 'save_profile') {
        const name = args.name.trim()
        const p = { name, scenes: JSON.parse(JSON.stringify(w.__scenes)) }
        const i = w.__profiles.findIndex((x) => x.name === name)
        if (i >= 0) w.__profiles[i] = p; else w.__profiles.push(p)
        emitProfiles()
        return name
      }
      if (cmd === 'apply_profile') {
        const sent = JSON.parse(JSON.stringify(findProfile(args.name).scenes))
        ;(w.__applied ??= []).push(args.name)
        w.__scenes = sent
        setTimeout(() => w.__emit('scenes', sent), 0)
        emitProfiles()
        return
      }
      if (cmd === 'rename_profile') {
        const to = args.to.trim()
        if (to !== args.from && w.__profiles.some((x) => x.name === to)) throw `There is already a profile called “${to}”`
        findProfile(args.from).name = to
        emitProfiles()
        return to
      }
      if (cmd === 'delete_profile') {
        findProfile(args.name)
        w.__profiles = w.__profiles.filter((x) => x.name !== args.name)
        emitProfiles()
        return
      }
      if (cmd === 'save_mode') {
        ;(w.__saves ??= []).push([args.device, JSON.parse(JSON.stringify(args.effect))])
        if (w.__rejectSave) throw w.__rejectSave
        return
      }
      if (cmd === 'set_segments') {
        ;(w.__segs ??= []).push([args.device, args.zone, JSON.parse(JSON.stringify(args.runs))])
        const d = w.__devices.find((x) => x.id === args.device)
        const z = d.zones[args.zone]
        let at = 0
        const segs = args.runs.map((r) => {
          const leds = Array.from({ length: r.leds }, (_, i) => z.start + at + i).filter((i) => i < d.colors.length)
          const p = { segment: { zone: args.zone, start: at }, name: r.name, leds }
          at += r.leds
          return p
        })
        const taken = new Set(segs.flatMap((p) => p.leds))
        const rest = d.colors.map((_, i) => i).filter((i) => !taken.has(i))
        d.pieces = segs.length ? [...(rest.length ? [{ segment: null, name: '', leds: rest }] : []), ...segs] : []
        setTimeout(() => w.__emit('devices', w.__devices), 0)
        return
      }
      if (cmd === 'resize_zone') {
        ;(w.__resizes ??= []).push([args.device, args.zone, args.size])
        if (w.__rejectResize) throw w.__rejectResize
        const d = w.__devices.find((x) => x.id === args.device)
        d.zones[args.zone].leds = args.size
        setTimeout(() => w.__emit('devices', w.__devices), w.__echoDelay ?? 0)
        return
      }
      if (cmd === 'set_scenes') {
        w.__calls.push(JSON.parse(JSON.stringify(args.scenes)))
        if (w.__reject) throw w.__reject
        w.__scenes = args.scenes
        const sent = args.scenes
        setTimeout(() => w.__emit('scenes', sent), w.__echoDelay ?? 0)
        emitProfiles()
        return
      }
      throw new Error('unexpected command ' + cmd)
    },
  }
}

let failed = 0
const ok = (cond, msg) => { console.log((cond ? 'PASS ' : 'FAIL ') + msg); if (!cond) failed++ }
const browser = await chromium.launch({ executablePath: EXE, args: ['--no-sandbox'] })
const page = await browser.newPage({ viewport: { width: 960, height: 640 } })
const errors = []
page.on('pageerror', (e) => errors.push(String(e)))
page.on('console', (m) => m.type() === 'error' && errors.push(m.text()))
await page.addInitScript(fakeTauri)
await page.goto(url)
await page.waitForSelector('.rail .t')

// The rail lists targets: All devices first, then each device. A device's panel shows what it
// is set to as the pressed look (vial), the pressed built-in mode, or neither (follows all).
const target = (i) => page.locator('.rail .t:not(.grp)').nth(i + 1) // -1 = All devices
const open = async (i) => { await target(i).click(); await page.waitForTimeout(60) }
const vial = (label) => page.locator('.gel', { hasText: label })
const speed = () => page.locator('.ctl label:has-text("Speed") input[type=range]')
const h1 = () => page.locator('.head h1').textContent()
const sub = () => page.locator('.head .sub').textContent()
/** What the open device is set to: a look's label, `mode:<name>`, or 'none'. */
const state = async () => {
  const v = page.locator('.gel[aria-pressed=true] .cap')
  if (await v.count()) return (await v.textContent()).trim()
  const modes = page.locator('[aria-labelledby=modes-label] .chip[aria-pressed=true]')
  if (await modes.count()) return 'mode:' + (await modes.textContent()).trim()
  return 'none'
}
const calls = () => page.evaluate(() => window.__calls)
const settle = () => page.waitForTimeout(400)
const types = (scenes) => scenes.map((s) => `${s.effect.type}@${s.targets === 'all' ? 'all' : s.targets.devices.map((d) => d.nth).join('+')}`)
const setRange = (loc, v) => loc.evaluate((el, v) => { el.value = String(v); el.dispatchEvent(new Event('input', { bubbles: true })) }, v)

const names = await page.locator('.rail .t .n').allTextContents()
ok(JSON.stringify(names) === '["All devices","Vengeance RGB #1","Vengeance RGB #2"]', `the rail lists All devices and both devices, numbered (two identical devices, distinct keys): ${JSON.stringify(names)}`)
ok((await calls()).length === 0, 'nothing is sent on load')
if (SHOTS) await page.screenshot({ path: `${SHOTS}/1-initial.png` })

// 1. give device 1 a look of its own
await open(0)
await vial('Wave').click()
await settle()
let c = await calls()
ok(c.length === 1 && JSON.stringify(types(c[0])) === '["wave@0"]', `picking Wave for device 1 sends one scene for that device only: ${JSON.stringify(c.map(types))}`)
ok(c[0][0].canvas === 'shared' && c[0][0].targets.devices[0].name === 'Vengeance RGB', 'scene names the device by its key')
await open(1)
ok((await state()) === 'none', 'device 2 is untouched')

// 2. slider change is debounced into one send, other device untouched
await open(0)
await setRange(speed(), 1.5)
await setRange(speed(), 1.6)
await setRange(speed(), 1.7)
await settle()
c = await calls()
ok(c.length === 2, `three quick slider moves send one scene (calls now ${c.length})`)
ok(c[1][0].effect.speed === 1.7, `the last value wins (${c[1]?.[0]?.effect.speed})`)

// 3. second device: first device's look is preserved
await open(1)
await vial('Rainbow').click()
await settle()
c = await calls()
ok(JSON.stringify(types(c.at(-1))) === '["wave@0","rainbow@1"]', `device 2's look is added next to device 1's: ${JSON.stringify(types(c.at(-1)))}`)
ok(c.at(-1)[0].effect.speed === 1.7, "device 1's settings are kept")
if (SHOTS) await page.screenshot({ path: `${SHOTS}/2-two-effects.png` })

// 4. a look for all devices replaces everything; devices follow
await open(-1)
await vial('Rainbow').click()
await settle()
c = await calls()
ok(JSON.stringify(types(c.at(-1))) === '["rainbow@all"]', `choosing a look replaces per-device looks: ${JSON.stringify(types(c.at(-1)))}`)
ok(/2 individual looks were replaced/.test(await page.locator('[role=status]').textContent()), 'the toast says what was replaced')
ok((await h1()) === 'Rainbow', 'the heading names the running look')
ok((await page.locator('.gel[aria-pressed=true]').textContent()).includes('Playing'), 'the running look is marked Playing')
await page.locator('[role=status] .toast').hover()
await page.waitForTimeout(7300)
ok((await page.getByRole('button', { name: 'Undo' }).count()) === 1, 'the toast stays while the pointer is on it')
await page.mouse.move(5, 5)
const nUndo = (await calls()).length
await page.keyboard.press('Control+z')
await settle()
c = await calls()
ok(c.length === nUndo + 1 && JSON.stringify(types(c.at(-1))) === '["wave@0","rainbow@1"]', `Ctrl+Z (Undo) sends the earlier list back: ${JSON.stringify(types(c.at(-1)))}`)
await open(0)
ok((await state()) === 'Wave', 'and the devices come back')
await open(-1)
await vial('Rainbow').click()
await settle()
await open(0)
const s0 = await state()
await open(1)
ok(s0 === 'none' && (await state()) === 'none', 'devices fall back to following all devices')
ok(/follows all devices \(Rainbow\)/.test(await sub()) && (await vial('Rainbow').textContent()).includes('Shared'), 'the device says what it follows, and that look is marked Shared')

// 5. own look on top of the all-devices look
await open(0)
await vial('Static colour').click()
await settle()
c = await calls()
ok(JSON.stringify(types(c.at(-1))) === '["rainbow@all","static@0"]', `own look sits on top of the all scene: ${JSON.stringify(types(c.at(-1)))}`)
ok(/Own look · Static colour/.test(await target(0).textContent()), 'the rail marks the device’s own look')
await open(-1)
ok(/1 has its own look/.test(await sub()), 'the All devices heading says one device has its own look')
// retuning the running look keeps the device's own look and keeps it winning
await setRange(page.locator('.ctl label:has-text("Brightness") input[type=range]'), 0.5)
await settle()
c = await calls()
ok(JSON.stringify(types(c.at(-1))) === '["rainbow@all","static@0"]' && c.at(-1)[0].effect.brightness === 0.5 && c.at(-1)[1].effect.brightness === 1, `retuning all devices changes only the all-devices scene: ${JSON.stringify(types(c.at(-1)))} ${c.at(-1)[0].effect.brightness}/${c.at(-1)[1].effect.brightness}`)
// back to "follow all" removes only the own look (the 0 key does the same as the button)
await open(0)
ok((await page.getByRole('button', { name: 'Follow all devices' }).count()) === 1, 'a device with its own look offers Follow all devices')
await page.locator('body').click({ position: { x: 600, y: 20 } })
await page.keyboard.press('0')
await settle()
c = await calls()
ok(JSON.stringify(types(c.at(-1))) === '["rainbow@all"]', 'pressing 0 (Follow all devices) removes only that device’s own look')
await page.keyboard.press('2')
await settle()
ok(JSON.stringify(types((await calls()).at(-1))) === '["rainbow@all","gradient@0"]', 'number keys pick a look for the device in view')
await page.keyboard.press('0')
await settle()

// 6. a slider edit still waiting when you leave the device is sent, not lost; Stop all after it wins
await open(1)
await vial('Wave').click()
await settle()
let before = (await calls()).length
await setRange(speed(), 0.5)
await open(-1)
await page.getByRole('button', { name: 'Stop all' }).click()
await settle()
c = await calls()
ok(c.length === before + 2 && c.at(-2).find((s) => s.targets !== 'all')?.effect.speed === 0.5 && JSON.stringify(c.at(-1)) === '[]', `leaving sends the pending edit, then Stop all clears everything (${c.length - before} sends)`)
ok((await h1()) === 'Effects are off', 'the heading says effects are off')
// a pending retune of all devices must not resurrect a look after Stop all
await page.getByRole('button', { name: 'Resume' }).click()
await settle()
ok(JSON.stringify(types((await calls()).at(-1))) === '["rainbow@all","wave@1"]', 'Resume brings back what Stop all switched off')
before = (await calls()).length
await setRange(page.locator('.ctl label:has-text("Brightness") input[type=range]'), 0.3)
await page.getByRole('button', { name: 'Stop all' }).click()
await settle()
c = await calls()
ok(JSON.stringify(c.at(-1)) === '[]' && c.length === before + 1, `Stop all wins over a pending slider edit (${c.length - before} sends after)`)
await open(1)
ok((await state()) === 'none', 'devices reset when effects are stopped')

// 7. a change from outside (another window, tray, paint) is followed
await open(0)
await vial('Breathing').click()
await settle()
await page.evaluate(() => { window.__scenes = []; window.__emit('scenes', []) })
await settle()
ok((await state()) === 'none', 'an outside change clears the device')
const n7 = (await calls()).length
await settle()
ok((await calls()).length === n7, 'following an outside change sends nothing back')

// 8. a look set outside shows up on its device with its settings
await page.evaluate(() => {
  const k = window.__devices[1].key
  const s = [{ effect: { type: 'wave', colors: ['#112233', '#445566'], speed: 0.9, density: 3, brightness: 0.5, direction: 'reverse' }, targets: { devices: [k] }, canvas: 'shared' }]
  window.__scenes = s; window.__emit('scenes', s)
})
await settle()
await open(1)
ok((await state()) === 'Wave', 'a look set elsewhere appears on its device')
ok((await speed().inputValue()) === '0.9', 'with its speed')
ok((await calls()).length === n7, 'and loading it sends nothing')

// 9. a rejected edit is shown and the device goes back
await page.evaluate(() => { window.__reject = 'brightness must be between 0 and 1' })
await open(0)
await vial('Static colour').click()
await settle()
ok(/brightness/.test(await page.locator('.error').textContent().catch(() => '')), 'a rejected scene shows the backend error')
await page.evaluate(() => { window.__reject = null })

// 10. the echo of our own edit arrives while the user is still dragging: the slider must not jump back
await page.evaluate(() => { window.__echoDelay = 60 })
await vial('Wave').click()
await settle()
const slider = speed()
await setRange(slider, 1.0)
await page.waitForTimeout(165) // debounce fires, scene sent, echo due in 60 ms
await setRange(slider, 1.2)
await page.waitForTimeout(120) // the echo of 1.0 has now arrived
ok((await slider.inputValue()) === '1.2', `a late echo of the previous value does not move the slider back (${await slider.inputValue()})`)
await settle()
c = await calls()
const mine = c.at(-1).find((x) => x.targets !== 'all' && x.targets.devices[0].nth === 0)
ok(mine?.effect.speed === 1.2, `and the final value is what ends up sent (${mine?.effect.speed})`)
await page.evaluate(() => { window.__echoDelay = 0 })

const font = await page.evaluate(async () => { await document.fonts.ready; return [document.fonts.check('14px "Geist Variable"'), document.fonts.check('11px "Geist Mono Variable"'), [...document.fonts].filter((f) => f.status === 'loaded').map((f) => f.family)] })
ok(font[0] && font[1] && font[2].includes('Geist Variable') && font[2].includes('Geist Mono Variable'), `the bundled fonts load (${JSON.stringify(font)})`)

// 10b. the device's built-in modes sit next to the looks
await page.evaluate(() => { window.__scenes = []; window.__emit('scenes', []) })
await settle()
const modes = await page.locator('[aria-labelledby=modes-label] .chip').allTextContents()
ok(JSON.stringify(modes) === '["Static","Breathing","Off"]', `the device's own modes are offered: ${JSON.stringify(modes)}`)
const base = (await calls()).length
await page.locator('[aria-labelledby=modes-label] .chip', { hasText: 'Breathing' }).click()
await settle()
c = await calls()
let m = c.at(-1)[0]
ok(c.length === base + 1 && m.effect.type === 'device_mode' && m.effect.mode === 'Breathing' && m.targets.devices[0].nth === 0, `picking a built-in mode sends one device_mode scene for that device: ${JSON.stringify(m.effect)}`)
ok(m.effect.speed === 0.5 && m.effect.brightness === 1 && m.effect.mode_direction === 'up' && JSON.stringify(m.effect.colors) === '["#ff0000","#0000ff"]', 'it starts from the values the device reports')
ok(!('density' in m.effect) && !('direction' in m.effect), 'and carries no software-only fields')
await setRange(speed(), 0.7); await setRange(speed(), 0.8)
await settle()
c = await calls()
m = c.at(-1)[0]
ok(c.length === base + 2 && m.effect.speed === 0.8, `speed is debounced into one send with the final position (${m.effect.speed}, ${c.length - base} sends)`)
await page.locator('.ctl label:has-text("Direction") select').selectOption('left')
await settle()
m = (await calls()).at(-1)[0]
ok(m.effect.mode_direction === 'left' && m.effect.speed === 0.8, 'changing the direction keeps the other settings')
const saveBtn = page.getByRole('button', { name: 'Save to device' })
ok((await saveBtn.count()) === 1, 'a mode the device can keep offers Save to device')
await setRange(speed(), 0.6)
await saveBtn.click()
await settle()
const saves = await page.evaluate(() => window.__saves ?? [])
m = (await calls()).at(-1)[0]
ok(saves.length === 1 && saves[0][1].mode === 'Breathing' && saves[0][1].speed === 0.6 && m.effect.speed === 0.6, `saving sends the pending slider edit first and saves those values: ${JSON.stringify(saves)}`)
ok((await page.locator('[role=status]').textContent()).includes('Saved “Breathing”'), 'and a message confirms it')
await page.evaluate(() => { window.__rejectSave = 'device busy' })
await saveBtn.click()
await settle()
ok(/Could not save to the device/.test(await page.locator('.error').textContent().catch(() => '')), 'a failed save says so')
await page.evaluate(() => { window.__rejectSave = undefined })
await vial('Wave').click()
await settle()
m = (await calls()).at(-1)[0]
ok(m.effect.type === 'wave' && !('mode' in m.effect) && m.effect.colors.length === 3 && m.effect.direction === 'forward', `switching to a look sends a clean software scene: ${JSON.stringify(m.effect)}`)
await page.locator('[aria-labelledby=modes-label] .chip', { hasText: 'Off' }).click()
await settle()
m = (await calls()).at(-1)[0]
ok(m.effect.mode === 'Off' && m.effect.colors.length === 0 && !('mode_direction' in m.effect), 'a mode with no settings sends none')
ok(/no settings/.test(await page.locator('.ctl').textContent()), 'and says so')
ok((await page.getByRole('button', { name: 'Save to device' }).count()) === 0, 'a mode the device cannot keep has no Save to device')
await page.evaluate(() => {
  const k = window.__devices[1].key
  const s = [{ effect: { type: 'device_mode', mode: 'Breathing', colors: ['#00ff00', '#0000ff'], speed: 0.3, brightness: 0.6, mode_direction: 'down' }, targets: { devices: [k] }, canvas: 'shared' }]
  window.__scenes = s; window.__emit('scenes', s)
})
await settle()
await open(1)
ok((await state()) === 'mode:Breathing' && (await speed().inputValue()) === '0.3', 'a built-in mode set elsewhere appears on its device with its settings')
const nb = (await calls()).length
await settle()
ok((await calls()).length === nb, 'and loading it sends nothing')
await page.evaluate(() => { window.__scenes = []; window.__emit('scenes', []) })
await settle()

// 10b2. per-zone modes: a fan hub whose channels can each run a built-in mode of their own
await page.evaluate(() => {
  window.__scenes = []; window.__emit('scenes', [])
  const none = { brightness: null, directions: [], direction: null, saveable: false, whole_device: false }
  const zm = [
    { ...none, index: 0, name: 'Fixed', speed: null, colors_min: 1, colors_max: 1, colors: ['#0080ff'] },
    { ...none, index: 1, name: 'Pulse', speed: 0.5, colors_min: 1, colors_max: 2, colors: ['#ff4000'] },
  ]
  window.__devices.push({
    id: 9, key: { name: 'Fan Hub', location: 'HID 9', serial: 'HUB1', nth: 0 }, name: 'Fan Hub', vendor: 'NZXT', device_type: 3, active_mode: 0,
    description: '', version: '', serial: 'HUB1', location: 'HID 9',
    modes: [
      { ...none, index: 1, name: 'Fixed', speed: null, colors_min: 1, colors_max: 1, colors: ['#ffffff'], saveable: true },
      { ...none, index: 3, name: 'Super Rainbow', speed: 0.5, colors_min: 0, colors_max: 0, colors: [], whole_device: true },
    ],
    zones: [0, 1, 2].map((i) => ({ index: i, name: `Fan ${i + 1}`, leds: 8, min_leds: 8, max_leds: 8, resizable: false, start: i * 8, matrix: null, modes: zm, active_mode: -1 })),
    colors: Array.from({ length: 24 }, () => '#203040'),
  })
  window.__emit('devices', window.__devices)
})
await settle()
await open((await page.locator('.rail .t:not(.grp)').count()) - 2)
ok((await h1()) === 'Fan Hub' && (await page.locator('#zones-label').count()) === 0, 'zones are offered only under a built-in mode')
await page.locator('[aria-labelledby=modes-label] .chip', { hasText: 'Fixed' }).click()
await settle()
const zchips = () => page.locator('[aria-labelledby=zones-label] .chip').allTextContents()
ok(JSON.stringify(await zchips()) === '["Whole device","Fan 1","Fan 2","Fan 3"]', `under a built-in mode the zones are listed: ${JSON.stringify(await zchips())}`)
await page.locator('[aria-labelledby=zones-label] .chip', { hasText: 'Fan 2' }).click()
ok(/Fan 2 follows the device’s mode \(Fixed\)/.test(await page.locator('.ctl').textContent()) && (await page.getByRole('button', { name: 'Save to device' }).count()) === 0, 'a zone starts out following the device, and saving is for the whole device')
const zb = (await calls()).length
await page.locator('[aria-labelledby=zone-modes-label] .chip', { hasText: 'Pulse' }).click()
await settle()
m = (await calls()).at(-1)[0]
ok((await calls()).length === zb + 1 && m.effect.mode === 'Fixed' && JSON.stringify(m.effect.zones) === '[{"zone":1,"mode":"Pulse","colors":["#ff4000"],"speed":0.5,"brightness":1}]', `picking a zone mode adds that zone's look to the device's: ${JSON.stringify(m.effect.zones)}`)
ok((await zchips())[2] === 'Fan 2Pulse', 'and the zone chip names its own mode')
await setRange(speed(), 0.8)
await settle()
m = (await calls()).at(-1)[0]
ok(m.effect.zones.length === 1 && m.effect.zones[0].speed === 0.8 && m.effect.speed === 0.5, 'the controls now tune the zone, not the device')
await page.locator('[aria-labelledby=zones-label] .chip', { hasText: 'Whole device' }).click()
ok((await page.locator('#zone-modes-label').count()) === 0 && (await page.locator('[aria-labelledby=modes-label] .chip[aria-pressed=true]').textContent()) === 'Fixed', 'Whole device goes back to the device\'s own modes')
await page.locator('[aria-labelledby=modes-label] .chip', { hasText: 'Super Rainbow' }).click()
await settle()
m = (await calls()).at(-1)[0]
ok(m.effect.mode === 'Super Rainbow' && m.effect.zones?.length === 1 && (await page.locator('#zones-label').count()) === 0 && /drives the whole device/.test(await page.locator('.ctl').textContent()), 'a mode that drives the whole device hides the zones but keeps their looks for later')
await page.locator('[aria-labelledby=modes-label] .chip', { hasText: 'Fixed' }).click()
await page.locator('[aria-labelledby=zones-label] .chip', { hasText: 'Fan 2' }).click()
await page.locator('[aria-labelledby=zone-modes-label] .chip', { hasText: 'Same as device' }).click()
await settle()
m = (await calls()).at(-1)[0]
ok(m.effect.mode === 'Fixed' && !('zones' in m.effect), `Same as device drops the zone's look: ${JSON.stringify(m.effect)}`)
await page.evaluate(() => {
  window.__devices = window.__devices.filter((d) => d.id !== 9)
  window.__scenes = []; window.__emit('scenes', []); window.__emit('devices', window.__devices)
})
await settle()

// 10c. zone size: only for zones the device lets us resize, one step deeper under More settings
await open(1)
ok((await page.locator('.ctl .zones').count()) === 0, 'no zone-size slider on a device without a resizable zone')
await open(0)
await vial('Wave').click()
await settle()
ok((await page.locator('.ctl .zones').count()) === 1 && (await page.locator('.ctl .more:has(.zones) .fold[inert]').count()) === 1, 'the zone-size slider is folded away under More settings')
await page.getByRole('button', { name: 'More settings' }).click()
ok((await page.locator('.ctl .more:has(.zones) .fold[inert]').count()) === 0, 'More settings opens it')
const resizes = () => page.evaluate(() => window.__resizes ?? [])
const zoneSlider = page.locator('.ctl .zones input[type=range]')
await setRange(zoneSlider, 100); await setRange(zoneSlider, 120); await setRange(zoneSlider, 150)
await settle()
let rz = await resizes()
ok(rz.length === 1 && rz[0][0] === 1 && rz[0][1] === 0 && rz[0][2] === 150, `dragging sends one resize with the last size: ${JSON.stringify(rz)}`)
ok(/150 LEDs/.test(await page.locator('.ctl .zones').textContent()), 'the count follows the device')
await page.evaluate(() => { window.__rejectResize = 'zone cannot be resized' })
await setRange(zoneSlider, 200)
await settle()
ok(/cannot be resized/.test(await page.locator('.error').textContent().catch(() => '')) && /150 LEDs/.test(await page.locator('.ctl .zones').textContent()), 'a refused resize shows the reason and the slider goes back')
await page.evaluate(() => { window.__rejectResize = null })

// 10c2. the stage: clicking a device on the matrix opens it (its shape is picked when arranging the desk)
await open(-1)
await page.waitForTimeout(900)
const box = await page.locator('.stage canvas').boundingBox()
let hit = null
for (let x = box.x + 20; x < box.x + box.width - 20 && !hit; x += 6) {
  for (const y of [box.y + box.height * 0.35, box.y + box.height * 0.5, box.y + box.height * 0.65]) {
    await page.mouse.click(x, y)
    if ((await h1()) !== 'All devices' && (await page.locator('.rail .t.on .n').textContent()) !== 'All devices') { hit = await h1(); break }
  }
}
ok(hit && hit.startsWith('Vengeance RGB'), `clicking a device on the stage opens it (${hit})`)

// 10d. painting by hand: a keyboard with a 4-LED logo zone in front of a 5x3 key grid
await page.evaluate(() => {
  window.__scenes = []; window.__emit('scenes', [])
  const cells = Array.from({ length: 15 }, (_, i) => (i === 10 ? null : 4 + (i > 10 ? i - 1 : i)))
  window.__devices.push({
    id: 3, key: { name: 'K70', location: 'HID 3', serial: '', nth: 0 }, name: 'K70', vendor: 'Corsair', device_type: 5, active_mode: 0, modes: [],
    zones: [
      { index: 0, name: 'Logo', leds: 4, min_leds: 4, max_leds: 4, resizable: false, start: 0, matrix: null },
      { index: 1, name: 'Keys', leds: 14, min_leds: 14, max_leds: 14, resizable: false, start: 4, matrix: { width: 5, height: 3, cells } },
    ],
    colors: Array.from({ length: 18 }, () => '#203040'),
  })
  window.__emit('devices', window.__devices)
})
await settle()
await open(2)
const painted = async () => (await calls()).at(-1).find((s) => s.targets !== 'all' && s.targets.devices[0].name === 'K70')
let n0 = (await calls()).length
const sheet = page.locator('dialog.sheet')
await page.getByRole('button', { name: 'Paint by hand' }).click()
await settle()
let p = await painted()
ok(await sheet.isVisible() && (await calls()).length === n0 + 1 && p?.effect.type === 'painted' && p.effect.colors.length === 18 && p.effect.colors.every((c) => c === '#203040'),
  `"Paint by hand" opens the painter and takes over the device with its current colours: ${JSON.stringify(p?.effect).slice(0, 80)}`)
ok((await sheet.locator('.grid > button').count()) === 14 && (await sheet.locator('.grid > span').count()) === 1 && (await sheet.locator('.strip > button').count()) === 4, 'the key grid and the logo row are drawn, with the empty cell')
await sheet.locator('input[type=color]').evaluate((el) => { el.value = '#00ff00'; el.dispatchEvent(new Event('input', { bubbles: true })) })
const centre = async (led) => { const b = await sheet.locator(`[data-led="${led}"]`).boundingBox(); return [b.x + b.width / 2, b.y + b.height / 2] }
const [x0, y0] = await centre(4), [x1, y1] = await centre(6)
n0 = (await calls()).length
// One jump: the painter must fill in the key it skipped.
await page.mouse.move(x0, y0); await page.mouse.down(); await page.mouse.move(x1, y1, { steps: 1 }); await page.mouse.up()
await settle()
p = await painted()
ok(['#00ff00', '#00ff00', '#00ff00', '#203040'].every((c, i) => p.effect.colors[4 + i] === c), `a fast drag paints every key it crosses: ${p.effect.colors.slice(4, 8)}`)
ok((await calls()).length - n0 <= 3, `a drag is sent a few times, not once per pixel (${(await calls()).length - n0})`)
await sheet.getByRole('button', { name: 'Fill Logo with the brush colour' }).click()
ok((await sheet.locator('.grid [tabindex="0"]').count()) === 1 && (await sheet.locator('.strip [tabindex="0"]').count()) === 1, 'each zone is one tab stop')
await sheet.locator('[data-led="6"]').focus()
await page.keyboard.press('ArrowDown'); await page.keyboard.press('ArrowRight')
const focused = await page.evaluate(() => document.activeElement?.dataset.led)
ok(focused === '12', `arrow keys move through the grid (focused LED ${focused})`)
await page.keyboard.press('Enter')
await settle()
p = await painted()
ok(p.effect.colors.slice(0, 4).every((c) => c === '#00ff00') && p.effect.colors[12] === '#00ff00' && p.effect.colors[11] === '#203040', 'Fill zone paints only that zone, and Enter paints a focused key')
ok(/LED 13, #00ff00/.test(await sheet.locator('[data-led="12"]').getAttribute('aria-label')), 'a key is announced with its colour')
await sheet.getByRole('button', { name: 'Off (black)' }).click()
ok((await sheet.getByRole('button', { name: '#00ff00', exact: true }).count()) === 1, 'a custom colour painted with stays on the swatch row after switching brush')
if (SHOTS) {
  const was = await page.evaluate(() => document.documentElement.dataset.theme)
  await page.evaluate(() => document.documentElement.dataset.theme = 'dark')
  await page.screenshot({ path: `${SHOTS}/paint-960.png` })
  await page.evaluate(() => document.documentElement.dataset.theme = 'light')
  await page.screenshot({ path: `${SHOTS}/paint-960-light.png` })
  await page.setViewportSize({ width: 390, height: 844 }); await settle()
  await page.screenshot({ path: `${SHOTS}/paint-mobile.png` })
  await page.setViewportSize({ width: 960, height: 640 }); await page.evaluate((t) => document.documentElement.dataset.theme = t, was); await settle()
}
await sheet.getByRole('button', { name: 'Done' }).click()
await settle()
ok(!(await sheet.isVisible()) && /Painting saved/.test(await page.locator('[role=status]').textContent()), 'Done closes the painter with an Undo toast')
ok(/Own look · Painted/.test(await target(2).textContent()) && (await page.getByRole('button', { name: 'Edit painting' }).count()) === 1, 'the device shows the painting and offers to edit it')
await page.getByRole('button', { name: 'Undo' }).click()
await settle()
ok(!(await painted()) && (await page.getByRole('button', { name: 'Paint by hand' }).count()) === 1, 'Undo takes the painting off again')
await page.getByRole('button', { name: 'Paint by hand' }).click()
await settle()
await sheet.locator('[data-led="5"]').click()
await page.keyboard.press('Escape')
await settle()
ok(!(await sheet.isVisible()) && !(await painted()) && (await state()) === 'none' && (await page.getByRole('button', { name: 'Paint by hand' }).count()) === 1, 'Escape cancels: the previous look comes back and the device follows')

// 10e. the painter at real sizes: a 23x6 keyboard with gaps and a 300-LED strip still fits the window
await page.evaluate(() => {
  const w = 23, h = 6
  const cells = []
  let led = 0
  for (let i = 0; i < w * h; i++) cells.push(i % 23 === 14 || i % 23 === 18 || (i >= 115 && i % 23 > 3 && i % 23 < 10) ? null : led++)
  window.__devices.push({
    id: 4, key: { name: 'Big board', location: 'HID 4', serial: '', nth: 0 }, name: 'Big board', vendor: 'Synthetic', device_type: 5, active_mode: 0, modes: [],
    zones: [
      { index: 0, name: 'Keys', leds: led, min_leds: led, max_leds: led, resizable: false, start: 0, matrix: { width: w, height: h, cells } },
      { index: 1, name: 'Underglow', leds: 300, min_leds: 300, max_leds: 300, resizable: false, start: led, matrix: null },
    ],
    colors: Array.from({ length: led + 300 }, (_, i) => `#${((i * 7) % 256).toString(16).padStart(2, '0')}30${((255 - i) & 255).toString(16).padStart(2, '0')}`),
  })
  window.__emit('devices', window.__devices)
})
await settle()
await open(3)
await page.getByRole('button', { name: 'Paint by hand' }).click()
await settle()
const inView = async (loc) => { const b = await loc.boundingBox(); return b && b.y >= 0 && b.y + b.height <= 640 }
const keyW = (await sheet.locator('.grid > button').first().boundingBox()).width
ok(await inView(sheet.getByRole('button', { name: 'Done' })) && keyW <= 38, `a full keyboard keeps Done in view and keys at most 38px (${keyW.toFixed(1)})`)
if (SHOTS) await page.screenshot({ path: `${SHOTS}/paint-big-960.png` })
await sheet.locator('[data-led]').last().scrollIntoViewIfNeeded()
const last = Number(await sheet.locator('[data-led]').last().getAttribute("data-led"))
const lb = await sheet.locator(`[data-led="${last}"]`).boundingBox(), pb = await sheet.locator(`[data-led="${last - 2}"]`).boundingBox()
await page.mouse.move(pb.x + pb.width / 2, pb.y + pb.height / 2); await page.mouse.down(); await page.mouse.move(lb.x + lb.width / 2, lb.y + lb.height / 2, { steps: 8 }); await page.mouse.up()
await settle()
const bp = (await calls()).at(-1).find((s) => s.targets !== 'all' && s.targets.devices[0].name === 'Big board')
ok(await inView(sheet.getByRole('button', { name: 'Done' })) && bp.effect.colors.length === last + 1 && bp.effect.colors.slice(-3).every((c) => c === bp.effect.colors.at(-1)) && bp.effect.colors.at(-4) !== bp.effect.colors.at(-1),
  'after scrolling to the end of the strip a drag paints its last LEDs, and Done stays in view')
if (SHOTS) await page.screenshot({ path: `${SHOTS}/paint-big-960-scrolled.png` })
await page.keyboard.press('Escape')
await settle()

// 11. theme toggle flips the theme and remembers it
const theme0 = await page.evaluate(() => document.documentElement.dataset.theme)
await page.getByRole('button', { name: /Switch to/ }).click()
await page.waitForFunction((t) => document.documentElement.dataset.theme !== t, theme0)
const theme1 = await page.evaluate(() => [document.documentElement.dataset.theme, localStorage.getItem('freergb-theme')])
ok(theme1[0] !== theme0 && theme1[0] === theme1[1], `theme toggles and is remembered (${theme0} -> ${theme1})`)
await page.getByRole('button', { name: /Switch to/ }).click()
await page.waitForFunction((t) => document.documentElement.dataset.theme === t, theme0)

// 12. a failed server shows plain text and Retry, and hides the looks
await page.evaluate(() => window.__emit('server-state', { kind: 'failed', reason: 'x', message: 'OpenRGB could not be started.' }))
await settle()
ok((await page.getByRole('button', { name: 'Retry' }).count()) >= 1 && (await page.locator('.gel').count()) === 0 && /could not be started/.test(await page.locator('.stage').textContent()), 'failed state says why, offers Retry and hides the looks')
await page.evaluate(() => window.__emit('server-state', { kind: 'connected', owned: false, server_name: 'OpenRGB (fake)' }))
await settle()

// 13. number keys pick a look for all devices
await open(-1)
await page.keyboard.press('6')
await settle()
ok(JSON.stringify(types((await calls()).at(-1))) === '["wave@all"]' && (await h1()) === 'Wave', 'pressing 6 puts Wave on every device')

// 13b. shelves: the Scenes shelf holds the new looks, keys pick from the open shelf, the shelf
// with the playing look is marked, and a look started elsewhere opens its shelf.
const tab = (name) => page.getByRole('tab', { name: new RegExp('^' + name) })
await tab('Scenes').click()
await settle()
const caps = await page.locator('.gel .cap').allTextContents()
ok(JSON.stringify(caps) === '["Comet","Marquee","Bouncing beam","Starfield","Fire","Rain"]', `the Scenes shelf holds six new looks: ${JSON.stringify(caps)}`)
ok((await tab('Essentials').locator('.dot').count()) === 1 && (await tab('Scenes').getAttribute('aria-selected')) === 'true', 'the shelf with the playing look carries a dot while another is open')
await page.keyboard.press('5')
await settle()
{
  const last = (await calls()).at(-1)
  ok(JSON.stringify(types(last)) === '["fire@all"]' && (await h1()) === 'Fire' && JSON.stringify(last[0].effect.colors) === '["#c81e00","#ff7a00","#ffd76a"]', `pressing 5 on Scenes starts Fire in its own palette: ${JSON.stringify(last[0]?.effect)}`)
}
await tab('Scenes').focus()
await page.keyboard.press('ArrowLeft')
await settle()
ok((await tab('Essentials').getAttribute('aria-selected')) === 'true' && (await page.evaluate(() => document.activeElement?.textContent?.trim().startsWith('Essentials'))), 'arrow keys move between shelves and focus follows')
await page.keyboard.press('6')
await settle()
ok((await h1()) === 'Wave' && (await tab('Essentials').getAttribute('aria-selected')) === 'true', 'and 6 there is Wave again')

// 13c. groups: make one from two devices in a chosen order, give it a look, see members follow
// it, send it back to following all, and delete it.
await page.getByRole('button', { name: 'New group of devices' }).click()
const groupSheet = page.locator('dialog.sheet[aria-labelledby=group-title]')
await groupSheet.waitFor()
ok((await groupSheet.locator('input.text').inputValue()) === 'Group 1' && (await groupSheet.getByRole('button', { name: 'Create group' }).isDisabled()), 'the new-group sheet suggests a name and needs devices first')
const devNames = await page.locator('.rail .t:not(.grp) .n').allTextContents()
await groupSheet.getByRole('button', { name: `Add ${devNames[1]}` }).click()
await groupSheet.getByRole('button', { name: `Add ${devNames[2]}` }).click()
await groupSheet.getByRole('button', { name: `Move ${devNames[2]} left` }).click()
await groupSheet.locator('input.text').fill('Desk strip')
if (SHOTS) await page.screenshot({ path: `${SHOTS}/group-sheet.png` })
await groupSheet.getByRole('button', { name: 'Create group' }).click()
await settle()
{
  const [cmd, args] = (await page.evaluate(() => window.__groupCalls)).at(-1)
  ok(cmd === 'save_group' && args.old === null && args.name === 'Desk strip' && args.devices.map((d) => d.nth).join() === '1,0', `creating sends the name and devices left to right: ${JSON.stringify(args)}`)
}
ok((await h1()) === 'Desk strip' && /follows all devices \(Wave\)/.test(await sub()), 'the new group opens in its own view, following all devices')
await vial('Breathing').click()
await settle()
{
  const last = (await calls()).at(-1)
  const gs = last.find((x) => x.group === 'Desk strip')
  ok(gs && gs.effect.type === 'breathing' && gs.targets.devices.map((d) => d.nth).join() === '1,0' && last[0].targets === 'all' && last.indexOf(gs) === 1, `a look on the group is one scene on its devices, after the all-devices look: ${JSON.stringify(types(last))}`)
}
ok((await page.locator('.rail .t.grp', { hasText: 'Desk strip' }).textContent()).includes('Group look · Breathing'), 'the rail marks the group look')
ok((await page.locator('.rail .t:not(.grp)').nth(1).textContent()).includes('In “Desk strip”'), 'and its devices as following it')
await open(0)
ok(/follows “Desk strip” \(Breathing\)/.test(await sub()) && (await page.locator('.gel', { hasText: 'Breathing' }).locator('.tag.ghost').count()) === 1, 'a member device follows the group look, marked Shared')
await page.locator('.rail .t.grp', { hasText: 'Desk strip' }).click()
await settle()
await page.keyboard.press('0')
await settle()
ok(!(await calls()).at(-1).some((x) => x.group) && /follows all devices/.test(await sub()), 'pressing 0 sends the group back to following all devices')
await page.getByRole('button', { name: 'Edit group' }).click()
await groupSheet.waitFor()
await groupSheet.getByRole('button', { name: 'Delete' }).click()
await groupSheet.getByRole('button', { name: 'Delete for good' }).click()
await settle()
ok((await page.evaluate(() => window.__groups.length)) === 0 && (await page.locator('.rail .t.grp').count()) === 0 && (await h1()) === 'Wave', 'deleting the group takes it out of the rail and returns to all devices')

// 13d. My effects: keep the playing look, import one, play it, export it, delete it.
await tab('My effects').click()
await settle()
ok(/Looks you keep appear here/.test(await page.locator('.gels').textContent()), 'an empty My effects shelf says how to fill it')
await page.getByRole('button', { name: 'Save this look', exact: true }).click()
const effectSheet = page.locator('dialog.sheet[aria-labelledby=effect-title]')
await effectSheet.waitFor()
ok((await effectSheet.locator('input.text').inputValue()) === 'Wave', 'the sheet suggests the look’s name')
await effectSheet.getByRole('button', { name: 'Save' }).click()
await settle()
ok((await page.evaluate(() => window.__saved.map((x) => x.name + ':' + x.effect.type).join())) === 'Wave:wave' && (await vial('Wave').locator('.tag').textContent()) === 'Playing', 'the kept look is a vial on the shelf, marked as playing')
ok((await page.getByRole('button', { name: 'Save this look', exact: true }).isDisabled()) && (await tab('Essentials').locator('.dot').count()) === 0, 'a kept look cannot be kept twice, and the dot does not point at another shelf')
await page.evaluate(() => (window.__importNext = { name: 'Embers', effect: { type: 'fire', colors: ['#400000', '#ff5000', '#ffd000'], speed: 0.6, density: 2, brightness: 0.8, direction: 'forward' } }))
await page.getByRole('button', { name: 'Import…' }).click()
await settle()
ok(/Imported “Embers”/.test(await page.locator('[role=status]').textContent()) && (await vial('Embers').count()) === 1, 'importing adds the effect to the shelf and says so')
await vial('Embers').click()
await settle()
{
  const last = (await calls()).at(-1)
  ok(last.length === 1 && last[0].targets === 'all' && JSON.stringify(last[0].effect) === JSON.stringify({ type: 'fire', colors: ['#400000', '#ff5000', '#ffd000'], speed: 0.6, density: 2, brightness: 0.8, direction: 'forward' }), `a saved vial plays its effect exactly as kept: ${JSON.stringify(last[0]?.effect)}`)
}
ok((await tab('My effects').getAttribute('aria-selected')) === 'true' && (await vial('Embers').locator('.tag').textContent()) === 'Playing', 'the shelf stays on My effects with the saved vial playing')
await page.keyboard.press('1')
await settle()
ok((await h1()) === 'Wave', 'number keys pick from My effects too')
await vial('Embers').hover()
await page.getByRole('button', { name: 'Edit Embers' }).click()
await effectSheet.waitFor()
await page.evaluate(() => (window.__exportTo = '/home/u/Embers.json'))
if (SHOTS) await page.screenshot({ path: `${SHOTS}/effect-sheet.png` })
await effectSheet.getByRole('button', { name: 'Export…' }).click()
await settle()
ok(JSON.stringify(await page.evaluate(() => window.__exported)) === '["Embers"]' && /Exported “Embers” to \/home\/u\/Embers.json/.test(await page.locator('[role=status]').textContent()) && !(await effectSheet.isVisible()), 'export writes the file the user picks, says where and closes the sheet')
if (SHOTS) {
  await page.waitForTimeout(600)
  await page.screenshot({ path: `${SHOTS}/mine-960.png` })
}
await vial('Embers').hover()
await page.getByRole('button', { name: 'Edit Embers' }).click()
await effectSheet.waitFor()
await effectSheet.getByRole('button', { name: 'Delete' }).click()
await effectSheet.getByRole('button', { name: 'Delete for good' }).click()
await settle()
ok((await page.evaluate(() => window.__saved.map((x) => x.name).join())) === 'Wave' && (await vial('Embers').count()) === 0, 'deleting takes it off the shelf')
await tab('Essentials').click()
await settle()

// 14. profiles: save the running look, see it marked as on, switch away and back with Undo,
// rename, and delete (two clicks).
const profileRow = (name) => page.locator('.rail .profiles .p', { hasText: name })
const profileSheet = page.locator('dialog.sheet[aria-labelledby=profile-title]')
ok(/Save a look here/.test(await page.locator('.rail .profiles').textContent()), 'with no profiles the rail says how to make one')
await page.getByRole('button', { name: 'Save this look as a profile' }).click()
ok(await profileSheet.isVisible() && (await profileSheet.locator('input.text').inputValue()) === 'Profile 1', 'the save sheet opens with a suggested name')
await profileSheet.locator('input.text').fill('  Night  ')
await page.keyboard.press('Enter')
await settle()
const waveAll = JSON.stringify((await calls()).at(-1))
ok(!(await profileSheet.isVisible()) && (await profileRow('Night').locator('.own').count()) === 1, 'Enter saves; the new profile is listed and marked as on')
ok(/Saved as “Night”/.test(await page.locator('.toast').textContent()), 'saving says so')
await page.keyboard.press('1')
await settle()
ok((await profileRow('Night').locator('.own').count()) === 0, 'changing the look clears the mark')
const nApply = (await calls()).length
await profileRow('Night').locator('.pa').click()
await settle()
ok(JSON.stringify(await page.evaluate(() => window.__applied)) === '["Night"]' && (await h1()) === 'Wave' && (await profileRow('Night').locator('.own').count()) === 1, 'clicking a profile applies it, the panel follows and the mark comes back')
ok((await calls()).length === nApply, 'applying a profile goes through the backend, not set_scenes')
await page.locator('.toast').getByRole('button', { name: 'Undo' }).click()
await settle()
ok(JSON.stringify(types((await calls()).at(-1))) !== JSON.stringify(types(JSON.parse(waveAll))) && (await profileRow('Night').locator('.own').count()) === 0, 'Undo puts back the look from before the profile')
await page.getByRole('button', { name: 'Save this look as a profile' }).click()
await profileSheet.locator('input.text').fill('Night')
ok(/Replaces the profile “Night”/.test(await profileSheet.textContent()) && (await profileSheet.getByRole('button', { name: 'Replace' }).count()) === 1, 'saving under a taken name warns and says Replace')
await page.keyboard.press('Escape')
await settle()
ok(!(await profileSheet.isVisible()) && (await page.evaluate(() => window.__profiles.length)) === 1, 'Escape closes without saving')
await profileRow('Night').hover()
await profileRow('Night').getByRole('button', { name: 'Edit profile Night' }).click()
await profileSheet.locator('input.text').fill('Late night')
await profileSheet.getByRole('button', { name: 'Save' }).click()
await settle()
ok(JSON.stringify(await page.evaluate(() => window.__profiles.map((p) => p.name))) === '["Late night"]', 'the edit sheet renames')
await profileRow('Late night').hover()
await profileRow('Late night').getByRole('button', { name: /Edit profile/ }).click()
await profileSheet.getByRole('button', { name: 'Delete' }).click()
ok((await page.evaluate(() => window.__profiles.length)) === 1 && (await profileSheet.getByRole('button', { name: 'Delete for good' }).count()) === 1, 'the first Delete only asks again')
await profileSheet.getByRole('button', { name: 'Delete for good' }).click()
await settle()
ok((await page.evaluate(() => window.__profiles.length)) === 0 && !(await profileSheet.isVisible()), 'the second click deletes')
await page.evaluate(() => window.__emit('notice', 'No profile called “Gone”'))
await settle()
ok(/No profile called/.test(await page.locator('.main .error').textContent()), 'a failed tray action shows in the window')
// 15. settings: starting at login is a switch backed by the OS entry
await page.getByRole('button', { name: 'Settings', exact: true }).click()
const login = page.getByRole('switch', { name: /Start when I log in/ })
await login.waitFor()
ok(!(await login.isChecked()), 'starting at login is off by default')
await login.click()
await settle()
ok((await page.evaluate(() => window.__autostart)) === true && (await login.isChecked()), 'the switch turns it on')
await page.keyboard.press('Escape')
await page.getByRole('button', { name: 'Settings', exact: true }).click()
await login.waitFor()
await settle()
ok(await login.isChecked(), 'reopened settings read it back')
const rate = (n) => page.getByRole('group', { name: 'Animation smoothness' }).getByRole('button', { name: `${n} fps` })
ok((await rate(30).getAttribute('aria-pressed')) === 'true' && (await rate(60).getAttribute('aria-pressed')) === 'false', 'effects run at 30 fps by default')
await rate(60).click()
await settle()
ok((await page.evaluate(() => window.__fps)) === 60 && (await rate(60).getAttribute('aria-pressed')) === 'true', 'choosing 60 fps sends it to the backend')
if (SHOTS) await page.screenshot({ path: `${SHOTS}/settings.png` })
await page.keyboard.press('Escape')

// Profiles for the screenshots.
await page.evaluate(() => {
  window.__profiles = [{ name: 'Evening', scenes: [] }, { name: 'Gaming', scenes: [] }, { name: 'Work', scenes: [] }]
  window.__emit('profiles', { names: ['Evening', 'Gaming', 'Work'], active: null })
})
await settle()

if (SHOTS) {
  await page.evaluate(() => { window.__scenes = []; window.__emit('scenes', []) })
  await vial('Rainbow').click()
  await open(0)
  await vial('Wave').click()
  await open(-1)
  await settle()
  await page.evaluate(() => document.documentElement.dataset.theme = 'dark')
  await page.waitForTimeout(800)
  await page.screenshot({ path: `${SHOTS}/user-960.png` })
  await page.screenshot({ path: `${SHOTS}/960-full.png`, fullPage: true })
  await page.setViewportSize({ width: 1440, height: 900 }); await page.waitForTimeout(800)
  await page.screenshot({ path: `${SHOTS}/desktop.png`, fullPage: true })
  await page.setViewportSize({ width: 390, height: 844 }); await page.waitForTimeout(800)
  await page.screenshot({ path: `${SHOTS}/mobile.png`, fullPage: true })
  await page.evaluate(() => document.documentElement.dataset.theme = 'light')
  await page.setViewportSize({ width: 960, height: 640 }); await page.waitForTimeout(800)
  await page.screenshot({ path: `${SHOTS}/960-light.png` })
  await page.getByRole('button', { name: 'Save this look as a profile' }).click()
  await page.waitForTimeout(300)
  await page.screenshot({ path: `${SHOTS}/profile-sheet-light.png` })
  await page.evaluate(() => document.documentElement.dataset.theme = 'dark')
  await page.waitForTimeout(300)
  await page.screenshot({ path: `${SHOTS}/profile-sheet.png` })
  await page.keyboard.press('Escape')
  // The Scenes shelf with Fire playing, dark and light, and its controls open.
  await page.getByRole('tab', { name: /^Scenes/ }).click()
  await vial('Fire').click()
  await settle()
  await page.waitForTimeout(1200)
  await page.screenshot({ path: `${SHOTS}/scenes-960.png`, fullPage: true })
  await page.locator('.ctl summary, .ctl button', { hasText: 'More settings' }).first().click().catch(() => {})
  await page.setViewportSize({ width: 1440, height: 900 }); await page.waitForTimeout(1200)
  await page.screenshot({ path: `${SHOTS}/scenes-desktop.png` })
  await page.evaluate(() => document.documentElement.dataset.theme = 'light')
  await page.setViewportSize({ width: 960, height: 640 }); await page.waitForTimeout(800)
  await page.screenshot({ path: `${SHOTS}/scenes-960-light.png`, fullPage: true })
  await page.setViewportSize({ width: 390, height: 844 }); await page.waitForTimeout(800)
  await page.screenshot({ path: `${SHOTS}/scenes-mobile.png`, fullPage: true })
  await page.evaluate(() => document.documentElement.dataset.theme = 'dark')
  await page.setViewportSize({ width: 960, height: 640 })
  // A group with a look of its own.
  await page.evaluate(() => {
    const keys = [window.__devices[3].key, window.__devices[0].key, window.__devices[1].key]
    window.__groups = [{ name: 'Desk strip', devices: keys }]
    window.__emit('groups', window.__groups)
  })
  await page.locator('.rail .t.grp').click()
  await page.getByRole('tab', { name: /^Scenes/ }).click()
  await vial('Comet').click()
  await settle()
  await page.waitForTimeout(1000)
  await page.screenshot({ path: `${SHOTS}/group-960.png` })
  await page.setViewportSize({ width: 1440, height: 900 }); await page.waitForTimeout(1000)
  await page.screenshot({ path: `${SHOTS}/group-desktop.png` })
  await page.setViewportSize({ width: 390, height: 844 }); await page.waitForTimeout(800)
  await page.screenshot({ path: `${SHOTS}/group-mobile.png`, fullPage: true })
  ok(await page.evaluate(() => document.querySelector('.head .acts').getBoundingClientRect().bottom <= document.querySelector('.stage').getBoundingClientRect().top), 'on a phone the group actions stay clear of the stage')
  await page.setViewportSize({ width: 960, height: 640 })
}

// The desk map: arrange on the stage, keep it with Undo, or cancel; the direction dial.
{
  await page.setViewportSize({ width: 1300, height: 820 })
  await page.evaluate(() => {
    window.__scenes = [{ effect: { type: 'rainbow', speed: 0.2, density: 1, brightness: 1, direction: 'forward' }, targets: 'all', canvas: 'shared' }]
    window.__emit('scenes', window.__scenes)
  })
  await target(-1).click()
  await settle()
  const mapCalls = () => page.evaluate(() => window.__mapCalls ?? 0)
  const before = await mapCalls()
  await page.getByRole('button', { name: 'Arrange your desk' }).click()
  await page.waitForTimeout(300)
  ok((await h1()) === 'Arrange your desk', 'Arrange opens the desk view')
  ok((await page.locator('.gel').count()) === 0, 'the vials step aside while arranging')
  await page.getByRole('button', { name: 'Cancel' }).click()
  await page.waitForTimeout(200)
  ok((await h1()) === 'Rainbow' && (await mapCalls()) === before, 'Cancel goes back and saves nothing')

  await page.getByRole('button', { name: 'Arrange your desk' }).click()
  await page.waitForTimeout(300)
  const map = page.getByRole('application', { name: /Desk map/ })
  await map.focus()
  await page.keyboard.press('ArrowRight') // picks the first device
  await page.waitForTimeout(100)
  ok((await page.locator('.who').textContent()).includes('Vengeance RGB #1'), `an arrow key picks the first device: ${await page.locator('.who').textContent()}`)
  await page.keyboard.press('ArrowDown')
  await page.keyboard.press('r')
  await page.keyboard.press('Tab')
  ok((await page.locator('.who').textContent()).includes('Vengeance RGB #2'), 'Tab on the map picks the next device')
  await page.locator('[aria-labelledby=arr-shape] .chip', { hasText: 'Keyboard' }).click()
  await page.getByRole('button', { name: 'Done' }).click()
  await settle()
  const saved = await page.evaluate(() => window.__map)
  const n = await page.evaluate(() => window.__devices.length)
  ok(saved?.length === n && saved.every((p) => p.at && p.shape), `Done saves every device with a spot and a drawing: ${JSON.stringify(saved?.map((p) => [p.shape, p.at]))}`)
  ok(saved?.[0].at.rot === 90 && saved?.[1].at.rot === 0, 'R turned the picked device a quarter turn')
  ok(saved?.[1].shape === 'keys', 'the picked device is drawn as chosen')
  // Boxes on the desk in cells (a placement is centre, cell span and rotation): none overlap,
  // and one empty dot stays between any two.
  const boxes = (saved ?? []).map(({ at }) => {
    const turnedSide = at.rot % 180 !== 0
    const w = (turnedSide ? at.h : at.w) + 1, h = (turnedSide ? at.w : at.h) + 1
    return { x0: at.x - (w - 1) / 2, y0: at.y - (h - 1) / 2, w, h }
  })
  const crowd = boxes.flatMap((a, i) => boxes.slice(i + 1).filter((b) => a.x0 < b.x0 + b.w + 1 && b.x0 < a.x0 + a.w + 1 && a.y0 < b.y0 + b.h + 1 && b.y0 < a.y0 + a.h + 1))
  ok(crowd.length === 0, `devices never overlap on the desk, even after a turn into a neighbour: ${JSON.stringify(boxes)}`)
  ok(saved?.[1].points?.length === 8, 'each device sends one LED spot per LED')
  ok((await h1()) === 'Rainbow', 'Done goes back to the all-devices view')
  await page.locator('.toast button', { hasText: 'Undo' }).click()
  await settle()
  ok(JSON.stringify(await page.evaluate(() => window.__map)) === '[]', 'Undo puts the previous desk back')

  // Segments: a strip split in Arrange becomes pieces placed by themselves.
  const stripWas = await page.evaluate(() => {
    const was = window.__devices[0].zones[0].leds
    window.__devices[0].zones[0].leds = 60
    window.__devices[0].colors = Array.from({ length: 60 }, () => '#ff8000')
    window.__emit('devices', window.__devices)
    return was
  })
  await page.getByRole('button', { name: 'Arrange your desk' }).click()
  await page.waitForTimeout(300)
  await map.focus()
  await page.keyboard.press('ArrowRight')
  await page.getByRole('button', { name: 'Split into segments' }).click()
  const segSheet = page.locator('dialog[open]')
  ok((await segSheet.locator('h2').textContent()) === 'Split into segments', 'the picked strip offers Split into segments')
  const segHint = () => segSheet.locator('.hint').textContent()
  ok(/LEDs 31–60 \(30\) stay with Vengeance RGB #1/.test(await segHint()), `a fresh split starts with half, the rest stays: ${await segHint()}`)
  const counts = segSheet.locator('input[type=number]')
  await counts.nth(0).fill('70')
  ok(/10 LEDs more/.test(await segHint()) && (await segSheet.getByRole('button', { name: 'Split' }).isDisabled()), 'too many LEDs is said and blocks the split')
  await counts.nth(0).fill('20')
  await segSheet.locator('input.nm').nth(0).fill('Desk')
  await segSheet.getByRole('button', { name: 'Add a segment' }).click()
  ok((await segSheet.locator('input.nm').nth(1).inputValue()) === 'Part 2' && (await counts.nth(1).inputValue()) === '40', 'an added segment takes what is left')
  ok(/use all 60 LEDs/.test(await segHint()), 'the sheet says when the segments use every LED')
  await segSheet.getByRole('button', { name: 'Split' }).click()
  await settle()
  const segs = await page.evaluate(() => window.__segs?.at(-1))
  ok(JSON.stringify(segs) === JSON.stringify([1, 0, [{ name: 'Desk', leds: 20 }, { name: 'Part 2', leds: 40 }]]), `Split sends the runs in order: ${JSON.stringify(segs)}`)
  ok(/Split “Vengeance RGB #1” into 2 segments/.test(await page.locator('.toast').textContent()), 'a toast confirms the split')
  ok((await h1()) === 'Arrange your desk', 'the split keeps arranging')
  await page.getByRole('button', { name: 'Done' }).click()
  await settle()
  const desk2 = await page.evaluate(() => window.__map)
  const segEntries = (desk2 ?? []).filter((p) => p.segment)
  ok(segEntries.length === 2 && segEntries.every((p) => p.at && p.points?.length) && segEntries[1].points.length === 40,
    `each segment is saved as its own piece with a spot: ${JSON.stringify(desk2?.map((p) => [p.segment, p.at && [p.at.x, p.at.y], p.points?.length]))}`)
  ok(!(desk2 ?? []).some((p) => !p.segment && p.device.name === 'Vengeance RGB' && p.device.nth === 0), `a strip split end to end leaves no piece of its own: ${JSON.stringify(desk2?.map((p) => [p.device.name, p.device.nth, p.segment, p.points?.length]))}`)
  const segBoxes = segEntries.map(({ at }) => ({ x0: at.x - at.w / 2, x1: at.x + at.w / 2, y0: at.y - at.h / 2, y1: at.y + at.h / 2 }))
  ok(!(segBoxes[0].x0 < segBoxes[1].x1 && segBoxes[1].x0 < segBoxes[0].x1 && segBoxes[0].y0 <= segBoxes[1].y1 && segBoxes[1].y0 <= segBoxes[0].y1), `the new pieces do not sit on each other: ${JSON.stringify(segBoxes)}`)
  // Joining back: the sheet opens on the zone's segments; removing them all joins the strip.
  await page.getByRole('button', { name: 'Arrange your desk' }).click()
  await page.waitForTimeout(300)
  await map.focus()
  await page.keyboard.press('ArrowRight')
  ok((await page.locator('.who').textContent()).includes('Vengeance RGB #1 · Desk'), `a segment is picked by its own name: ${await page.locator('.who').textContent()}`)
  await page.getByRole('button', { name: 'Edit segments' }).click()
  ok((await segSheet.locator('input.nm').nth(0).inputValue()) === 'Desk', 'Edit segments starts from the saved ones')
  await segSheet.getByRole('button', { name: 'Remove Part 2' }).click()
  await segSheet.getByRole('button', { name: 'Remove Desk' }).click()
  await segSheet.getByRole('button', { name: 'Join back' }).click()
  await settle()
  ok(JSON.stringify(await page.evaluate(() => window.__segs?.at(-1))) === JSON.stringify([1, 0, []]), 'Join back clears the segments')
  await page.getByRole('button', { name: 'Cancel' }).click()
  await page.evaluate((was) => {
    window.__devices[0].zones[0].leds = was
    window.__devices[0].colors = window.__devices[0].colors.slice(0, 8)
    window.__devices[0].pieces = []
    window.__map = []
    window.__emit('devices', window.__devices)
  }, stripWas)
  await settle()

  const dial = page.getByRole('slider', { name: 'Direction across the desk' })
  ok((await dial.getAttribute('aria-valuenow')) === '0', 'the dial shows the effect’s own direction (rainbow runs left to right)')
  await dial.focus()
  await page.keyboard.press('ArrowRight')
  await page.keyboard.press('ArrowRight')
  await settle()
  let last = (await calls()).at(-1)
  ok(last?.[0].effect.angle === 30, `the dial sends the angle in 15° steps: ${JSON.stringify(last?.[0].effect)}`)
  await page.locator('.aim button').click()
  await settle()
  last = (await calls()).at(-1)
  ok(last?.[0].effect.angle === undefined, 'its reset button goes back to the effect’s own direction')
  // Reverse runs it the other way, and the dial says so.
  await page.evaluate(() => {
    window.__scenes = [{ ...window.__scenes[0], effect: { ...window.__scenes[0].effect, direction: 'reverse' } }]
    window.__emit('scenes', window.__scenes)
  })
  await settle()
  ok((await dial.getAttribute('aria-valuenow')) === '180', 'with Reverse on the dial shows where the look really moves')
  await dial.focus()
  await page.keyboard.press('ArrowRight')
  await settle()
  last = (await calls()).at(-1)
  ok(last?.[0].effect.angle === 15 && last?.[0].effect.direction === 'reverse', `turning it keeps Reverse and sends the angle it stands for: ${JSON.stringify(last?.[0].effect)}`)
  await page.locator('.aim button').click()
  await settle()
  last = (await calls()).at(-1)
  ok(last?.[0].effect.angle === undefined && last?.[0].effect.direction === 'forward', 'the reset button also clears Reverse')
  await page.setViewportSize({ width: 960, height: 640 })
}

// Follows: a look's setting follows a reading this machine offers.
{
  await target(-1).click()
  await page.evaluate(() => {
    window.__scenes = [{ effect: { type: 'static', colors: ['#0000ff'], speed: 0.25, density: 1, brightness: 1, direction: 'forward' }, targets: 'all', canvas: 'shared' }]
    window.__emit('scenes', window.__scenes)
  })
  await settle()
  const follows = page.locator('label.field', { hasText: 'Follows' })
  const select = follows.locator('select')
  const options = await select.locator('option').allTextContents()
  ok(JSON.stringify(options) === JSON.stringify(['Nothing', 'CPU temperature', 'Sound level', 'Screen colours']), `Follows offers this machine's sources: ${options}`)
  await select.selectOption('cpu_temp')
  await settle()
  let last = (await calls()).at(-1)?.[0].effect
  ok(JSON.stringify(last?.react) === JSON.stringify({ source: 'cpu_temp', drives: 'color' }) && last?.colors.length === 2,
    `a temperature moves the colour first, with a low and a high colour: ${JSON.stringify(last)}`)
  ok((await follows.locator('b').textContent()) === 'No reading yet', 'with no reading yet the control says so')
  ok((await follows.locator('.hint').textContent()) === 'Needs LibreHardwareMonitor running.', 'and what the reading needs, under the choice')
  ok((await page.locator('.field .lbl', { hasText: 'Colours, low to high' }).count()) === 1, 'the colours read low to high')
  await page.evaluate(() => window.__emit('readings', { cpu_temp: 61.6 }))
  await page.waitForTimeout(150)
  ok((await follows.locator('b').textContent()) === '62 °C', 'the reading shows live next to the label')
  ok((await follows.locator('.hint').count()) === 0, 'the note goes once a reading arrives')
  await page.getByRole('group', { name: 'Moves' }).getByRole('button', { name: 'Brightness' }).click()
  await settle()
  last = (await calls()).at(-1)?.[0].effect
  ok(last?.react?.drives === 'brightness' && last?.colors.length === 1, `Brightness: the look keeps its one colour: ${JSON.stringify(last)}`)
  const toggle = page.locator('.more .toggle').first()
  if ((await toggle.getAttribute('aria-expanded')) !== 'true') await toggle.click()
  await page.waitForTimeout(600)
  const low = page.getByRole('spinbutton', { name: 'Low end' })
  if (await low.isVisible()) {
    await low.fill('45')
    await low.press('Tab')
    await settle()
    last = (await calls()).at(-1)?.[0].effect
    ok(last?.react?.low === 45, `the reading range is set in the unit shown: ${JSON.stringify(last?.react)}`)
  } else ok(false, 'the reading range sits in More settings')
  ok((await page.locator('label.field', { hasText: 'Sensitivity' }).count()) === 0, 'a temperature offers no sensitivity')
  await select.selectOption('sound')
  await settle()
  const sens = page.locator('label.field', { hasText: 'Sensitivity' }).locator('input')
  const strength = page.locator('label.field', { hasText: 'Strength' }).locator('input')
  ok((await sens.inputValue()) === '50' && (await strength.inputValue()) === '100', 'a sound look offers sensitivity at 50% and strength at 100%')
  await sens.fill('80')
  await strength.fill('40')
  await settle()
  last = (await calls()).at(-1)?.[0].effect
  ok(last?.react?.sensitivity === 0.8 && last?.react?.strength === 0.4, `sensitivity and strength go with the look: ${JSON.stringify(last?.react)}`)
  await strength.fill('100')
  await settle()
  last = (await calls()).at(-1)?.[0].effect
  ok(last?.react && !('strength' in last.react), `the usual strength is left unset: ${JSON.stringify(last?.react)}`)
  await select.selectOption('screen')
  await settle()
  last = (await calls()).at(-1)?.[0].effect
  ok(last?.react?.source === 'screen' && last.react.drives === 'color' && (await page.getByText('From the screen').count()) === 1, 'the screen gives each LED its colour')
  await select.selectOption('')
  await settle()
  last = (await calls()).at(-1)?.[0].effect
  ok(last && !('react' in last), 'Nothing follows nothing')
  await page.evaluate(() => { window.__scenes = []; window.__emit('scenes', []); window.__emit('readings', {}) })
  await settle()
}

// Rescan: the rail's button asks OpenRGB to look again; the status line shows progress and the
// detector, and a toast says what was found.
{
  await target(-1).click()
  const btn = page.getByRole('button', { name: 'Look for devices again' })
  ok((await btn.count()) === 1, 'the rail offers Look for devices again while connected')
  await page.evaluate(() => { window.__holdScan = true })
  await btn.click()
  await page.waitForTimeout(450)
  const status = await page.locator('.rail .status .full').textContent()
  ok(status === 'Looking for devices…', `the status line says a scan runs: ${status}`)
  ok((await page.locator('.rail .scan .what').textContent()) === '40% · Corsair Vengeance', 'how far it got and the detector running are under it')
  ok(await btn.isDisabled(), 'the button rests while a scan runs')
  await btn.click({ force: true }).catch(() => {})
  ok((await page.evaluate(() => window.__rescans)) === 1, 'a second press during a scan asks nothing')
  await page.evaluate(() => {
    window.__holdScan = false
    window.__devices = [...window.__devices, { ...window.__devices[0], id: 9, key: { name: 'Lighting Node', location: 'HID 9', serial: '', nth: 0 }, name: 'Lighting Node' }]
    window.__emit('devices', window.__devices)
    const d = { scanning: false, percent: null, label: '' }
    window.__detection = d
    window.__emit('detection', d)
  })
  await page.waitForTimeout(800)
  ok((await page.locator('.toast').textContent())?.includes('Found a new device: Lighting Node'), `the toast names the new device: ${await page.locator('.toast').textContent()}`)
  ok((await page.locator('.rail .status .full').textContent()).startsWith('Connected to'), 'the status line goes back to the connection')
  ok((await page.locator('.rail .scan').count()) === 0, 'and the progress line is gone')

  // About this device: what OpenRGB reports, empty fields left out.
  await target(0).click()
  await page.getByRole('button', { name: 'About this device' }).click()
  await page.waitForTimeout(600)
  const rows = await page.locator('.about dt').allTextContents()
  ok(JSON.stringify(rows) === '["Type","Vendor","Description","Version","Location","Zone"]', `the info panel lists the reported fields, no empty serial: ${JSON.stringify(rows)}`)
  ok((await page.locator('.about').textContent()).includes('I2C: /dev/i2c-0, address 0x58'), 'with the device’s location')
  ok(/Strip\s*\d+ LEDs/.test(await page.locator('.about').textContent()), `and each zone’s LED count: ${await page.locator('.about').textContent()}`)
  await page.evaluate(() => {
    window.__devices = window.__devices.filter((d) => d.id !== 9)
    window.__emit('devices', window.__devices)
  })
  await target(-1).click()
}

// Rename or hide: freeRGB's own settings. A hidden device leaves the window and every look.
{
  await target(0).click()
  await page.getByRole('button', { name: 'Rename or hide' }).click()
  const sheet = page.locator('dialog[open]')
  ok((await sheet.locator('input.text').getAttribute('placeholder')) === 'Vengeance RGB', 'the name field shows the device’s own name as its placeholder')
  await sheet.locator('input.text').fill('Top stick')
  await sheet.getByRole('button', { name: 'Save' }).click()
  await settle()
  const names = await page.locator('.rail .t:not(.grp) .n').allTextContents()
  ok(names[1] === 'Top stick' && names[2] === 'Vengeance RGB', `the new name shows in the rail; the other stick needs no number now: ${JSON.stringify(names)}`)
  ok((await page.locator('h1').textContent()) === 'Top stick', 'and as the device’s heading')
  ok((await page.locator('.toast').textContent()).includes('Renamed to “Top stick”'), 'a toast confirms the rename')
  await page.getByRole('button', { name: 'Undo' }).click()
  await settle()
  ok((await page.locator('.rail .t:not(.grp) .n').nth(1).textContent()) === 'Vengeance RGB #1', 'Undo brings the old name back')

  const before = await page.locator('.rail .t:not(.grp)').count()
  await target(1).click()
  await page.getByRole('button', { name: 'Rename or hide' }).click()
  await sheet.getByRole('switch', { name: /Hide from freeRGB/ }).check()
  await sheet.getByRole('button', { name: 'Hide' }).click()
  await settle()
  const left = await page.locator('.rail .t:not(.grp) .n').allTextContents()
  ok(left.length === before - 1 && left[1] === 'Vengeance RGB' && !left.includes('Vengeance RGB #2'), `the hidden device leaves the rail: ${JSON.stringify(left)}`)
  ok((await page.locator('h1').textContent()) !== 'Vengeance RGB #2', 'and the window goes back to all devices')
  const call = (await page.evaluate(() => window.__noteCalls)).at(-1)
  ok(call.hidden === true && call.device.nth === 1 && call.name === null, `hiding sends the device’s key: ${JSON.stringify(call)}`)
  ok((await page.locator('.toast').textContent()).includes('Bring it back from Settings'), 'the toast says where to bring it back')

  await page.getByRole('button', { name: 'Settings' }).first().click()
  const settings = page.locator('dialog[open]')
  ok((await settings.locator('.hidden li').count()) === 1, 'Settings lists the hidden device')
  await settings.getByRole('button', { name: 'Show' }).click()
  await settle()
  ok((await settings.locator('.hidden').count()) === 0, 'Show brings it back and the list goes')
  await settings.getByRole('button', { name: 'Close' }).click()
  ok((await page.locator('.rail .t:not(.grp)').count()) === before, 'the device is back in the rail')
}

// Schedule: times of day and sleep/wake, edited in a sheet from the rail.
{
  await page.evaluate(() => {
    window.__profiles = [{ name: 'Work', scenes: [] }]
    window.__emit('profiles', { names: ['Work'], active: null })
  })
  const plate = page.locator('.rail .sched')
  ok(/No set times/.test(await plate.textContent()) && /Sleep: lights off · Wake: look from before/.test(await plate.textContent()), `the rail shows the defaults: ${await plate.textContent()}`)
  await page.getByRole('button', { name: 'Edit the schedule' }).click()
  const sheet = page.locator('dialog.sheet[aria-labelledby=schedule-title]')
  ok(await sheet.isVisible(), 'the pencil opens the schedule sheet')
  await sheet.getByRole('button', { name: 'Add a time' }).click()
  ok((await sheet.locator('input.time').inputValue()) === '23:00', 'a new time starts at 23:00, lights off')
  await sheet.locator('input.time').fill('07:30')
  await sheet.locator('input.time').dispatchEvent('change')
  await sheet.getByRole('combobox', { name: /^At / }).selectOption('p:Work')
  await sheet.getByRole('button', { name: 'Saturday' }).click()
  await sheet.getByRole('button', { name: 'Sunday' }).click()
  await sheet.getByRole('combobox', { name: 'When it wakes' }).selectOption('nothing')
  await sheet.getByRole('button', { name: 'Save' }).click()
  await settle()
  const saved = await page.evaluate(() => window.__schedule)
  ok(JSON.stringify(saved) === JSON.stringify({ rules: [{ at: 450, days: 31, action: { kind: 'profile', name: 'Work' } }], sleep: { kind: 'off' }, wake: { kind: 'nothing' } }), `Save sends the schedule: ${JSON.stringify(saved)}`)
  ok(!(await sheet.isVisible()), 'and closes')
  const text = await plate.textContent()
  ok(/07:30/.test(text) && /Work/.test(text) && /Weekdays/.test(text) && /Wake: nothing/.test(text), `the rail lists it: ${text}`)

  await page.getByRole('button', { name: 'Edit the schedule' }).click()
  for (const d of ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday']) await sheet.getByRole('button', { name: d }).click()
  ok(/Pick a day/.test(await sheet.textContent()) && (await sheet.getByRole('button', { name: 'Save' }).isDisabled()), 'a time with no day cannot be saved')
  await sheet.getByRole('button', { name: /^Remove 07:30/ }).click()
  ok(/No times yet/.test(await sheet.textContent()), 'removing the last time says how to add one')
  await sheet.getByRole('button', { name: 'Cancel' }).click()
  ok((await page.evaluate(() => window.__schedule.rules.length)) === 1, 'Cancel keeps what was saved')
}

// Diagnostics: OpenRGB's log (turned on from here) and the hardware it sees, from Settings.
{
  await page.getByRole('button', { name: 'Settings' }).click()
  await page.locator('dialog.sheet[aria-labelledby=settings-title]').getByRole('button', { name: 'Open Diagnostics' }).click()
  const sheet = page.locator('dialog.sheet[aria-labelledby=diag-title]')
  ok(await sheet.isVisible(), 'Settings opens the diagnostics sheet')
  ok(!(await page.locator('dialog.sheet[aria-labelledby=settings-title]').isVisible()), 'in place of Settings')
  await settle()
  ok(/not keeping a log/.test(await sheet.textContent()), 'with the log off, the sheet says so')
  await sheet.getByRole('button', { name: 'Turn on OpenRGB’s log' }).click()
  await sheet.locator('ol.log li').first().waitFor({ timeout: 3000 })
  ok((await sheet.locator('ol.log li').count()) === 3, 'once on, the usual lines show (debug hidden)')
  await sheet.getByRole('combobox', { name: 'Show' }).selectOption({ label: 'Problems' })
  ok((await sheet.locator('ol.log li').count()) === 2, 'Problems shows warnings and errors')
  await page.evaluate(() => {
    window.__log.lines.push({ seq: 5, level: 1, time_ms: 3000, file: 'x', line: 1, text: 'live error' })
    window.__emit('server-log', null)
  })
  await page.waitForTimeout(100)
  ok(/live error/.test(await sheet.locator('ol.log').textContent()), 'a new line arrives live')
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write'])
  await sheet.getByRole('button', { name: 'Copy' }).click()
  const clip = await page.evaluate(() => navigator.clipboard.readText())
  ok(clip.startsWith('[0:01.834] [Warning]') && clip.endsWith('live error'), `Copy puts the shown lines on the clipboard: ${clip}`)
  await sheet.getByRole('button', { name: 'Clear' }).click()
  await page.waitForTimeout(60)
  ok(/No lines yet/.test(await sheet.textContent()), 'Clear empties the log')

  await sheet.getByRole('tab', { name: 'Hardware' }).click()
  await sheet.getByText('SMBus I801 adapter at efa0').waitFor({ timeout: 2000 })
  const hw = await sheet.textContent()
  ok(/8086:7A23 \(1462:7D25\)/.test(hw) && /046D:C52B/.test(hw) && /usage 0001:0006/.test(hw) && /1A86:7523/.test(hw), 'the hardware tab lists buses, USB, HID and serial ports with hex ids')
  await sheet.getByRole('button', { name: 'Copy' }).click()
  ok((await page.evaluate(() => navigator.clipboard.readText())).startsWith('SMBus buses (1)'), 'Copy puts the hardware list on the clipboard')
  await page.keyboard.press('Escape')
  ok(!(await sheet.isVisible()), 'Escape closes it')

  // An OpenRGB freeRGB did not start: the setting waits for the user to restart it.
  await page.evaluate(() => { window.__log = { epoch: 9, state: 'off', owned: false, lines: [] } })
  await page.getByRole('button', { name: 'Settings' }).click()
  await page.locator('dialog.sheet[aria-labelledby=settings-title]').getByRole('button', { name: 'Open Diagnostics' }).click()
  await settle()
  ok(/next time OpenRGB starts/.test(await sheet.textContent()), 'for a foreign OpenRGB it says when the setting takes effect')
  await sheet.getByRole('button', { name: 'Turn on OpenRGB’s log' }).click()
  await page.waitForTimeout(100)
  ok(/Restart OpenRGB to start its log/.test(await sheet.textContent()), 'and then asks for a restart')
  await page.keyboard.press('Escape')
}

// Lights off: every device black, with Undo; the next look does not start from black.
{
  await target(-1).click()
  await page.evaluate(() => {
    window.__scenes = [{ effect: { type: 'wave', colors: ['#ff0000', '#00ff00', '#0000ff'], speed: 0.5, density: 1, brightness: 1, direction: 'forward' }, targets: 'all', canvas: 'shared' }]
    window.__emit('scenes', window.__scenes)
  })
  await settle()
  await page.getByRole('button', { name: 'Lights off', exact: true }).click()
  await settle()
  let last = (await calls()).at(-1)
  ok(last?.length === 1 && last[0].targets === 'all' && last[0].effect.type === 'static' && JSON.stringify(last[0].effect.colors) === '["#000000"]', `Lights off sends one black look for every device: ${JSON.stringify(last)}`)
  ok((await page.locator('h1').textContent()) === 'Lights are off' && (await target(-1).textContent()).includes('Lights off'), 'the heading and the rail say the lights are off')
  ok((await page.getByRole('button', { name: 'Lights off', exact: true }).count()) === 0, 'and the button steps aside')
  ok((await page.getByRole('button', { name: 'Stop all', exact: true }).count()) === 0, 'Stop all hides too: it would light the devices again')
  ok((await page.locator('.toast').textContent()).includes('Lights off'), 'a toast offers Undo')
  await vial('Breathing').click()
  await settle()
  last = (await calls()).at(-1)
  ok(last?.[0].effect.type === 'breathing' && !last[0].effect.colors.includes('#000000'), `a look picked after Lights off is lit: ${JSON.stringify(last?.[0].effect.colors)}`)
}

// Updates: the rail says one is ready; Settings shows what's new and installs it.
{
  ok((await page.getByRole('button', { name: 'Update ready' }).count()) === 0, 'no update, no word of one')
  await page.evaluate(() => {
    window.__update = { version: '0.1.0-beta.2', notes: '- Updates install from the app\n- Stop all stays hidden while dark' }
    window.__emit('update', window.__update)
  })
  await settle()
  await page.getByRole('button', { name: 'Update ready' }).click()
  const settings = page.locator('dialog.sheet[aria-labelledby=settings-title]')
  const text = await settings.textContent()
  ok(text.includes('freeRGB 0.1.0-beta.2 is ready') && text.includes('Stop all stays hidden while dark'), 'Update ready opens Settings with the version and what is new')
  await settings.getByRole('button', { name: 'Install' }).click()
  await page.waitForTimeout(60)
  ok((await settings.textContent()).includes('Downloading · 40%') && (await settings.getByRole('button', { name: 'Install' }).isDisabled()), 'installing shows the download and holds the button')
  await page.waitForTimeout(250)
  ok((await settings.locator('.error').textContent()).includes('connection reset') && !(await settings.getByRole('button', { name: 'Install' }).isDisabled()), 'a failed download says why and can be tried again')
  await page.keyboard.press('Escape')
}

// About: from Settings and from the tray; links go through the backend by name.
{
  await page.getByRole('button', { name: 'Settings', exact: true }).click()
  const settings = page.locator('dialog.sheet[aria-labelledby=settings-title]')
  await settings.getByRole('button', { name: 'Open About freeRGB' }).click()
  const about = page.locator('dialog.sheet[aria-labelledby=about-title]')
  await settle()
  const t = await about.textContent()
  ok(t.includes('0.1.0-beta.1') && t.includes('A front-end for OpenRGB') && t.includes('OpenRGB 1.1, under the same licence'), 'About shows the version, the OpenRGB credit and the bundled OpenRGB')
  ok(!(await settings.isVisible()), 'in place of Settings')
  await about.getByRole('button', { name: 'GitHub Sponsors' }).click()
  await about.getByRole('button', { name: 'Ko-fi' }).click()
  await about.getByRole('button', { name: /Source code/ }).click()
  ok(JSON.stringify(await page.evaluate(() => window.__links)) === '["sponsor","kofi","source"]', 'links open by name')
  await about.getByRole('button', { name: 'Third-party notices' }).click()
  ok((await about.locator('pre.well').textContent()).includes('SIL OPEN FONT LICENSE'), 'the notices ship with the app')
  await about.getByRole('button', { name: 'GNU GPL text' }).click()
  ok((await about.locator('pre.well').textContent()).includes('GNU GENERAL PUBLIC LICENSE') && (await about.locator('pre.well').count()) === 1, 'and the GPL, one at a time')
  await about.getByRole('button', { name: 'Open-source libraries' }).click()
  await page.waitForTimeout(150)
  ok(/svelte \d/.test(await about.locator('pre.well').textContent()), 'and every library the build bundled')
  if (SHOTS) {
    for (const th of ['dark', 'light']) {
      await page.evaluate((th) => (document.documentElement.dataset.theme = th), th)
      await page.screenshot({ path: `${SHOTS}/about-${th}.png` })
    }
    await about.getByRole('button', { name: 'Open-source libraries' }).click()
    await page.screenshot({ path: `${SHOTS}/about-closed.png` })
    await page.setViewportSize({ width: 390, height: 760 })
    await page.screenshot({ path: `${SHOTS}/about-mobile.png` })
    await page.setViewportSize({ width: 960, height: 640 })
  }
  await page.keyboard.press('Escape')
  ok(!(await about.isVisible()), 'Escape closes it')
  await page.evaluate(() => { window.__aboutAsked = true; window.__emit('about', null) })
  await settle()
  ok(await about.isVisible(), 'the tray opens it in an open window')
  ok((await page.evaluate(() => window.__aboutAsked)) === false, 'and the request is taken, so a new window does not open it again')
  await page.keyboard.press('Escape')
}

// Start over: asks once more in place, then resets and reloads the window.
{
  await page.getByRole('button', { name: 'Settings', exact: true }).click()
  const settings = page.locator('dialog.sheet[aria-labelledby=settings-title]')
  await settings.getByRole('button', { name: 'Reset', exact: true }).click()
  ok(/cannot be undone/.test(await settings.textContent()), 'Reset asks once more, saying it cannot be undone')
  await settings.getByRole('button', { name: 'Cancel' }).click()
  ok((await settings.getByRole('button', { name: 'Reset', exact: true }).count()) === 1, 'Cancel backs out')
  await settings.getByRole('button', { name: 'Reset', exact: true }).click()
  await Promise.all([page.waitForEvent('load'), settings.getByRole('button', { name: 'Reset for good' }).click()])
  await page.waitForSelector('.rail .t')
  ok((await page.evaluate(() => sessionStorage.getItem('reset'))) === '1', 'Reset for good resets and the window starts over')
}

// Linux, no devices and no udev rules: the stage and Diagnostics offer to set up access.
{
  await page.evaluate(() => { sessionStorage.setItem('udev', 'missing'); sessionStorage.setItem('nodevices', '1') })
  await page.reload()
  await page.waitForSelector('.stage .notice')
  await settle()
  const notice = page.locator('.stage .notice')
  ok(/permission to reach your RGB devices/.test(await notice.textContent()), 'no devices and no rules: the stage says why it may be')
  const shoot = async (name) => {
    if (!SHOTS) return
    for (const t of ['dark', 'light']) {
      await page.evaluate((t) => (document.documentElement.dataset.theme = t), t)
      await page.screenshot({ path: `${SHOTS}/${name}-${t}.png` })
    }
    await page.setViewportSize({ width: 390, height: 760 })
    await page.screenshot({ path: `${SHOTS}/${name}-mobile.png` })
    await page.setViewportSize({ width: 960, height: 640 })
  }
  await shoot('access-stage')
  await page.getByRole('button', { name: 'Settings', exact: true }).click()
  await page.locator('dialog.sheet[aria-labelledby=settings-title]').getByRole('button', { name: 'Open Diagnostics' }).click()
  const diag = page.locator('dialog.sheet[aria-labelledby=diag-title]')
  await diag.getByRole('tab', { name: 'Hardware' }).click()
  ok((await diag.getByRole('button', { name: 'Set up device access' }).count()) === 1, 'Diagnostics offers it above the hardware list')
  await settle()
  await shoot('access-diag')
  await page.keyboard.press('Escape')
  await notice.getByRole('button', { name: 'Set up device access' }).click()
  await page.waitForTimeout(80)
  ok(/cancelled/.test(await page.locator('.toast').textContent()), 'a cancelled password prompt says so')
  await notice.getByRole('button', { name: 'Set up device access' }).click()
  await page.waitForTimeout(80)
  ok((await page.locator('.toast').textContent()).includes('Device access is set up'), 'set up: it says so')
  await page.waitForTimeout(1600)
  ok((await page.evaluate(() => window.__rescans)) === 1 && (await page.locator('.rail .t .n').allTextContents()).includes('Lighting Node'), 'and OpenRGB looks again, finding the devices')
  await page.evaluate(() => sessionStorage.removeItem('nodevices'))
}

// Windows: the window has no system title bar; the UI's own sits in the top edge.
{
  await page.evaluate(() => sessionStorage.setItem('frame', '1'))
  await page.reload()
  await page.waitForSelector('.frame')
  await settle()
  const frame = page.locator('.frame')
  ok((await frame.getAttribute('data-tauri-drag-region')) !== null, 'framed: the top edge drags the window')
  await frame.getByRole('button', { name: 'Minimise' }).click()
  await frame.getByRole('button', { name: 'Close' }).click()
  ok((await page.evaluate(() => window.__winCalls.join())) === 'minimize,close', 'minimise and close ask the window')
  await frame.getByRole('button', { name: 'Maximise' }).click()
  await page.evaluate(() => window.__emit('tauri://resize', { width: 1, height: 1 }))
  await page.waitForTimeout(50)
  ok((await frame.getByRole('button', { name: 'Restore' }).count()) === 1, 'maximised: the button turns to Restore')
  const cap = await frame.getByRole('button', { name: 'Restore' }).boundingBox()
  const head = await page.locator('.main .head').first().boundingBox()
  ok(cap.y + cap.height <= head.y, 'the heading starts below the caption buttons')
  const main = await page.locator('.main').boundingBox()
  ok(main.y >= cap.y + cap.height, 'the scrolling column (and its scrollbar) starts below them')
  if (SHOTS) {
    await page.mouse.move(cap.x + 20, cap.y + 12)
    for (const t of ['dark', 'light']) {
      await page.evaluate((t) => (document.documentElement.dataset.theme = t), t)
      await page.screenshot({ path: `${SHOTS}/frame-${t}.png` })
    }
    await page.setViewportSize({ width: 390, height: 760 })
    await page.screenshot({ path: `${SHOTS}/frame-mobile.png` })
    await page.setViewportSize({ width: 1400, height: 860 })
    await page.screenshot({ path: `${SHOTS}/frame-wide.png` })
    await page.setViewportSize({ width: 960, height: 640 })
  }
  await page.evaluate(() => sessionStorage.removeItem('frame'))
}

ok(errors.length === 0, `no console or page errors ${errors.join(' | ')}`)
await browser.close(); server.close()
console.log(failed ? `\n${failed} FAILED` : '\nall passed')
process.exit(failed ? 1 : 0)
