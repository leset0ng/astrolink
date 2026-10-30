const assert = require("node:assert/strict")
const fs = require("node:fs")
const path = require("node:path")
const vm = require("node:vm")
const { test } = require("node:test")

function pageWithBrightness(setKeepScreenOn) {
  const source = fs.readFileSync(path.join(__dirname, "../src/pages/probe/probe.ux"), "utf8")
  const script = source.match(/<script>([\s\S]*?)<\/script>/)[1]
    .replace(/^import .*$/gm, "")
    .replace("export default", "globalThis.page =")
  const calls = { started: 0, destroyed: 0 }
  class NativeProbeRunner {
    start() { calls.started++; return Promise.resolve() }
    destroy() { calls.destroyed++; return Promise.resolve() }
  }
  const context = vm.createContext({
    brightness: { setKeepScreenOn }, NativeProbeRunner,
    console: { log() {}, warn() {}, error() {} }
  })
  vm.runInContext(script, context)
  const page = { ...context.page.private, ...context.page }
  return { page, calls }
}

test("probe starts when keep-screen-on never invokes its callbacks", () => {
  const settings = []
  const { page, calls } = pageWithBrightness(options => settings.push(options.keepScreenOn))
  page.onInit()
  assert.equal(calls.started, 1)
  assert.deepEqual(settings, [true])
  page.onDestroy()
  assert.equal(calls.destroyed, 1)
  assert.deepEqual(settings, [true, false])
})

test("an unavailable screen policy does not block the native probe", () => {
  const { page, calls } = pageWithBrightness(() => { throw new Error("UNSUPPORTED") })
  page.onInit()
  assert.equal(calls.started, 1)
})
