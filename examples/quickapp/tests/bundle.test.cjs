const assert = require("node:assert/strict")
const fs = require("node:fs")
const path = require("node:path")
const vm = require("node:vm")
const { test } = require("node:test")
const { primitive } = require("./primitive.cjs")

function environment(loadFeature, native = {}) {
  const global = { setTimeout, clearTimeout, setInterval, clearInterval }
  const context = vm.createContext({ console, ...native, globalThis: global, $app_require$: loadFeature })
  function load(relative) {
    const source = fs.readFileSync(path.join(__dirname, "../build", relative), "utf8")
    const wrapper = vm.runInContext(`(${source.replace(/^export default /, "")})`, context)
    const module = {}
    wrapper(global, global, null, module, null)
    if (module.entry) {
      const exported = {}
      module.entry(exported)
      return exported.default
    }
    return module.default
  }
  return { global, load }
}

test("compiled page lazily loads the declared Feature and shows a missing-module error", () => {
  const manifest = JSON.parse(fs.readFileSync(path.join(__dirname, "../build/manifest.json"), "utf8"))
  let calls = 0
  const env = environment(name => {
    assert.equal(name, `@app-module/${manifest.features[0].name}`)
    calls++
    throw new Error("FEATURE_NOT_FOUND")
  })
  const app = env.load("app.js")
  const page = env.load("pages/index/index.js")
  assert.equal(calls, 0)
  const state = { ...page.data, ...page }
  state.onInit()
  assert.equal(calls, 1)
  assert.equal(state.ready, false)
  assert.equal(state.status, "模块不可用")
  app.onDestroy()
  assert.equal(env.global.__astrolinkExampleController, undefined)
})

test("separately bundled app and page share one primitive subscription and Session cleanup", async () => {
  const raw = primitive()
  let required = 0
  const env = environment(() => { required++; return raw.feature })
  const app = env.load("app.js")
  const page = env.load("pages/index/index.js")
  const state = { ...page.data, ...page }
  state.onInit()
  assert.equal(raw.sessions.size, 1)
  assert.equal(required, 1)
  assert.equal(raw.subscriptions, 1)
  assert.equal(state.ready, true)
  app.onDestroy()
  state.onDestroy()
  await Promise.resolve()
  assert.equal(raw.requests.length, 1)
  assert.equal(raw.requests[0].op, 8)
  assert.equal(raw.unsubscriptions, 0)
  raw.complete()
  for (let i = 0; i < 12; i++) await Promise.resolve()
  assert.equal(raw.unsubscriptions, 1)
  assert.equal(env.global.__astrolinkExampleController, undefined)
  assert.equal(env.global.__canopusAstrolinkFacade, undefined)
})

test("legacy primitive on the real global survives a separate wrapper globalThis", async () => {
  const raw = primitive()
  let required = 0
  const env = environment(() => { required++; return null }, {
    __canopusAstrolinkNative: raw.feature
  })
  const app = env.load("app.js")
  const page = env.load("pages/index/index.js")
  const state = { ...page.data, ...page }
  state.onInit()
  assert.equal(state.ready, true)
  assert.equal(required, 0)
  assert.equal(raw.sessions.size, 1)
  assert.equal(raw.subscriptions, 1)
  app.onDestroy()
  state.onDestroy()
  await Promise.resolve()
  raw.complete()
  for (let i = 0; i < 12; i++) await Promise.resolve()
  assert.equal(raw.unsubscriptions, 1)
  assert.equal(env.global.__canopusAstrolinkFacade, undefined)
})

test("built page scans, connects and verifies Ping/max-length through numeric primitive events", async () => {
  const raw = primitive()
  const request = raw.feature.request
  raw.feature.request = (...args) => {
    const result = request(...args)
    const latest = raw.requests.at(-1)
    queueMicrotask(() => {
      if (latest.op === 5) {
        raw.complete(latest, { link: 42, peerId: "00112233445500", maxMessageBytes: 512 })
      } else {
        raw.complete(latest)
        if (latest.op === 6 && latest.data.byteLength >= 9) {
          const response = new Uint8Array(latest.data.slice(0))
          response[4] = 2
          raw.emit({ kind: 4, link: 42, data: response.buffer })
        }
      }
    })
    return result
  }
  const env = environment(() => raw.feature)
  const app = env.load("app.js")
  const page = env.load("pages/index/index.js")
  const state = { ...page.data, ...page }
  state.onInit()
  await state.toggleScan()
  assert.equal(state.scanning, true)
  raw.emit({ kind: 2, peerId: "00112233445500", rssi: -40, data: new Uint8Array([161]).buffer })
  assert.equal(state.peers.length, 1)
  await state.connectPeer(state.peers[0].peerId)
  assert.equal(state.connected, true)
  await state.sendPing()
  assert.match(state.result, /Ping 回包通过/)
  await state.runTests()
  assert.match(state.result, /均通过/)
  assert.equal(state.sent, 5)
  assert.equal(state.received, 3)
  const sends = raw.requests.filter(request => request.op === 6)
  assert.equal(sends.length, 5)
  assert.equal(sends[2].data.byteLength, 512)
  assert.equal(sends[3].data.byteLength, 0)
  assert.deepEqual(Array.from(new Uint8Array(sends[4].data)), [0xa5])
  app.onDestroy()
  for (let i = 0; i < 16; i++) await Promise.resolve()
  assert.equal(raw.unsubscriptions, 1)
})
