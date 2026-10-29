const assert = require("node:assert/strict")
const { test } = require("node:test")
const { TestController } = require("../.test-build/controller")
const { makeProbe, parseProbe, makeResponse, verifyResponse } = require("../.test-build/probe")
const { utf8ByteLength } = require("@canopus/astrolink")

// Test-only native fixtures exercise app behavior; they are never bundled in the RPK.
function fixture() {
  let advertisement, connection, data, disconnect
  const removed = []
  const sent = []
  const link = {
    id: "link-1", peerId: "peer-1", maxMessageBytes: 512,
    onData(callback) { data = callback; return () => removed.push("data") },
    onDisconnect(callback) { disconnect = callback; return () => removed.push("disconnect") },
    async send(value) {
      const bytes = new Uint8Array(value.buffer || value, value.byteOffset || 0, value.byteLength)
      if (bytes.length > this.maxMessageBytes) throw Object.assign(new Error("too large"), { code: "INVALID_ARGUMENT" })
      sent.push(new Uint8Array(bytes))
      const packet = parseProbe(bytes.slice().buffer)
      if (packet && !packet.response) queueMicrotask(() => data(makeResponse(packet).buffer))
    },
    async disconnect() { disconnect("DISCONNECTED") }
  }
  const calls = []
  const session = {
    onAdvertisement(callback) { advertisement = callback; return () => removed.push("advertisement") },
    onConnection(callback) { connection = callback; return () => removed.push("connection") },
    async startScan() { calls.push("startScan") }, async stopScan() { calls.push("stopScan") },
    async startAdvertising(options) { calls.push(["startAdvertising", options]) },
    async stopAdvertising() { calls.push("stopAdvertising") },
    async connect(peerId) { calls.push(["connect", peerId]); return link },
    async close() { calls.push("close") }
  }
  const native = {
    getCapabilities() { return { supported: true, maxLinks: 1, maxServiceIdBytes: 6, maxAdvertisingDataBytes: 1 } },
    createSession(options) {
      if (!options.serviceId || Buffer.byteLength(options.serviceId) > 6) {
        throw Object.assign(new Error("invalid UTF-8 byte length"), { code: "INVALID_ARGUMENT" })
      }
      if (options.serviceId === "chat") return session
      calls.push(["temporarySession", options.serviceId])
      return { ...session, close: async () => calls.push("temporaryClose") }
    }
  }
  const controller = new TestController(() => native)
  controller.initialize()
  return {
    controller, session, native, link, sent, calls, removed,
    advertise: event => advertisement(event), incoming: () => connection(link),
    receive: value => data(value), drop: reason => disconnect(reason)
  }
}

function deferred() {
  let resolve
  const promise = new Promise(done => { resolve = done })
  return { promise, resolve }
}

test("echo envelope verifies every binary byte and refuses malformed packets", () => {
  const request = makeProbe(0xf1234567, 512)
  const parsed = parseProbe(request.buffer)
  assert.equal(parsed.sequence, 0xf1234567)
  assert.equal(parsed.response, false)
  const response = makeResponse(parsed)
  assert.equal(parseProbe(response.buffer).response, true)
  assert.ok(verifyResponse(request, response))
  response[257] ^= 1
  assert.equal(verifyResponse(request, response), false)
  assert.equal(parseProbe(new Uint8Array(4).buffer), null)
  request[3] = 2
  assert.equal(parseProbe(request.buffer), null)
})

test("native absence and unsupported firmware remain visible failures", () => {
  const absent = new TestController(() => { throw new Error("Feature missing") })
  absent.initialize()
  assert.equal(absent.snapshot().ready, false)
  assert.equal(absent.snapshot().status, "模块不可用")
  assert.match(absent.snapshot().log[0].message, /Feature missing/)
  const unsupported = new TestController(() => ({ getCapabilities: () => ({ supported: false }) }))
  unsupported.initialize()
  assert.equal(unsupported.snapshot().status, "当前固件不支持")
})

test("scan reports preserve user bytes and bound peer history", async () => {
  const f = fixture()
  await f.controller.toggleScan()
  assert.equal(f.controller.snapshot().scanning, true)
  for (let i = 0; i < 20; i++) f.advertise({ peerId: `peer-${i}`, serviceId: "chat", rssi: -i,
    data: new Uint8Array([0, 128, 255]).buffer })
  f.advertise({ peerId: "peer-19", serviceId: "chat", rssi: -5, data: new Uint8Array([1]).buffer })
  const view = f.controller.snapshot()
  assert.equal(view.peers.length, 12)
  assert.equal(view.peers[0].rssi, -5)
  assert.equal(view.peers[1].dataHex, "00 80 ff")
  view.peers[0].peerId = "changed snapshot"
  assert.equal(f.controller.snapshot().peers[0].peerId, "peer-19")
  await f.controller.destroy()
})

test("advertising and scan stop only their own Session operations", async () => {
  const f = fixture()
  await f.controller.toggleAdvertising()
  assert.equal(f.controller.snapshot().advertising, true)
  assert.deepEqual(f.calls[0][1].data, new Uint8Array([0xa1]))
  await f.controller.toggleAdvertising()
  await f.controller.toggleScan()
  await f.controller.toggleScan()
  assert.equal(f.controller.snapshot().advertising, false)
  assert.equal(f.controller.snapshot().scanning, false)
  await f.controller.destroy()
})

test("Ping and max-SDU tests require actual byte-identical echo and oversize rejection", async () => {
  const f = fixture()
  await f.controller.connect("peer-1")
  await f.controller.ping()
  assert.match(f.controller.snapshot().result, /Ping 回包通过/)
  await f.controller.testBinaryAndLimits()
  const view = f.controller.snapshot()
  assert.equal(view.sent, 5)
  assert.equal(view.received, 3)
  assert.match(view.result, /均通过/)
  assert.equal(f.sent[2].byteLength, 512)
  assert.equal(f.sent[3].byteLength, 0)
  assert.deepEqual(Array.from(f.sent[4]), [0xa5])
  await f.controller.disconnect()
  assert.equal(f.controller.snapshot().connected, false)
  await f.controller.destroy()
  assert.deepEqual(f.removed.sort(), ["advertisement", "connection", "data", "disconnect"].sort())
})

test("incoming request is echoed once; its response never loops", async () => {
  const f = fixture()
  f.incoming()
  const packet = makeProbe(123, 100)
  f.receive(packet.buffer)
  await Promise.resolve()
  assert.equal(f.sent.length, 1)
  assert.equal(parseProbe(f.sent[0].buffer).response, true)
  f.receive(f.sent[0].buffer)
  await Promise.resolve()
  assert.equal(f.sent.length, 1)
  await f.controller.destroy()
})

test("closing while scan is pending invalidates late completion and callbacks", async () => {
  const f = fixture()
  const start = deferred()
  f.session.startScan = () => start.promise
  const scanning = f.controller.toggleScan()
  await f.controller.closeSession()
  start.resolve()
  await scanning
  f.advertise({ peerId: "late", serviceId: "chat", data: new ArrayBuffer(0), rssi: -1 })
  assert.equal(f.controller.snapshot().ready, false)
  assert.equal(f.controller.snapshot().scanning, false)
  assert.equal(f.controller.snapshot().peers.length, 0)
  assert.equal(f.controller.snapshot().busy, "")
  assert.ok(f.calls.includes("close"))
})

test("disconnect rejects an in-flight echo without retaining its timer", async () => {
  const f = fixture()
  await f.controller.connect("peer-1")
  f.link.send = async () => {}
  const ping = f.controller.ping()
  f.drop("RECEIVE_OVERFLOW")
  await ping
  assert.equal(f.controller.snapshot().connected, false)
  assert.match(f.controller.snapshot().result, /RECEIVE_OVERFLOW/)
  await f.controller.destroy()
})

test("logs stay bounded and a closed Session can be recreated", async () => {
  const f = fixture()
  for (let i = 0; i < 30; i++) await f.controller.toggleScan()
  assert.equal(f.controller.snapshot().log.length, 20)
  await f.controller.closeSession()
  f.controller.reopen()
  assert.equal(f.controller.snapshot().ready, true)
  await f.controller.destroy()
  f.controller.reopen()
  assert.equal(f.controller.snapshot().ready, false)
})


test("UTF-8 service-name boundary uses native byte limits and closes its temporary Session", async () => {
  assert.equal(utf8ByteLength("chat"), 4)
  assert.equal(utf8ByteLength("中文"), 6)
  assert.equal(utf8ByteLength("中文a"), 7)
  assert.equal(utf8ByteLength("a😀"), 5)
  assert.equal(utf8ByteLength("\ud800"), 3)
  const f = fixture()
  await f.controller.testServiceNames()
  assert.deepEqual(f.calls[0], ["temporarySession", "中中"])
  assert.equal(f.calls[1], "temporaryClose")
  assert.match(f.controller.snapshot().result, /6 B 通过，7 B 已拒绝/)
  assert.equal(f.controller.snapshot().ready, true)
  await f.controller.destroy()
})


test("a synchronously throwing native close does not leave the reopen gate stuck", async () => {
  const f = fixture()
  f.session.close = () => { throw new Error("native close failed") }
  await f.controller.closeSession()
  assert.equal(f.controller.snapshot().status, "Session 关闭失败")
  assert.equal(f.controller.snapshot().busy, "")
  f.controller.reopen()
  assert.equal(f.controller.snapshot().ready, true)
  await f.controller.destroy()
})
