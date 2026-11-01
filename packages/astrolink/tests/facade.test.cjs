const assert = require("node:assert/strict")
const { test } = require("node:test")
const { AstrolinkFacade, loadAstrolink, disposeAstrolink, OP, EVENT } = require("../dist/cjs/index.cjs")
const { primitive } = require("./primitive.cjs")
const turn = async () => { await Promise.resolve(); await Promise.resolve() }
const incoming = { kind: EVENT.CONNECTION, link: 42, peerId: "00112233445500", maxMessageBytes: 512 }

function setup() {
  const raw = primitive()
  const facade = new AstrolinkFacade(raw.feature)
  const session = facade.createSession({ serviceId: "chat" })
  return { raw, facade, session }
}
async function close(f) {
  const promise = f.session.close()
  f.raw.complete()
  await promise
  await f.facade.dispose()
}

test("native loader selects the resident legacy primitive and retains the Feature fallback", async () => {
  const previousLoader = globalThis.$app_require$
  const previousNative = globalThis.__canopusAstrolinkNative
  const previousAiot = globalThis.aiot
  const core = primitive()
  const legacy = primitive()
  const feature = primitive()
  let calls = 0
  try {
    globalThis.aiot = { canopusAstrolink: core.feature }
    globalThis.__canopusAstrolinkNative = legacy.feature
    globalThis.$app_require$ = () => { throw new Error("legacy loader must not require a Feature") }
    assert.equal(loadAstrolink(), loadAstrolink())
    await disposeAstrolink()
    assert.equal(core.subscriptions, 1)
    assert.equal(core.unsubscriptions, 1)
    assert.equal(legacy.subscriptions, 0)
    delete globalThis.aiot
    loadAstrolink()
    await disposeAstrolink()
    assert.equal(legacy.subscriptions, 1)
    assert.equal(legacy.unsubscriptions, 1)
    delete globalThis.__canopusAstrolinkNative
    globalThis.$app_require$ = name => {
      assert.equal(name, "@app-module/canopus.astrolink")
      calls++
      return feature.feature
    }
    assert.equal(loadAstrolink(), loadAstrolink())
    await disposeAstrolink()
    assert.equal(calls, 1)
    assert.equal(feature.unsubscriptions, 1)
  } finally {
    await disposeAstrolink()
    if (previousLoader === undefined) delete globalThis.$app_require$
    else globalThis.$app_require$ = previousLoader
    if (previousAiot === undefined) delete globalThis.aiot
    else globalThis.aiot = previousAiot
    if (previousNative === undefined) delete globalThis.__canopusAstrolinkNative
    else globalThis.__canopusAstrolinkNative = previousNative
  }
})

test("primitive ABI creates numeric sessions, validates UTF-8 and subscribes once", async () => {
  const f = setup()
  assert.equal(f.raw.sessions.get(11), "chat")
  assert.equal(f.raw.subscriptions, 1)
  assert.throws(() => f.facade.createSession({ serviceId: "a".repeat(65) }), { code: "INVALID_ARGUMENT" })
  const second = f.facade.createSession({ serviceId: "中文" })
  assert.equal(f.raw.sessions.get(12), "中文")
  const closing = second.close()
  f.raw.complete()
  await closing
  await close(f)
  assert.equal(f.raw.subscriptions, 1)
  assert.equal(f.raw.unsubscriptions, 1)
})

test("request tokens settle on matching complete events and preserve immediate/native error codes", async () => {
  const f = setup()
  let done = false
  const start = f.session.startScan({ allowDuplicates: true }).then(() => { done = true })
  const request = f.raw.requests.at(-1)
  assert.equal(request.op, OP.START_SCAN)
  assert.equal(request.handle, 11)
  assert.equal(request.flag, true)
  f.raw.emit({ token: request.token, session: 99 })
  await turn()
  assert.equal(done, false)
  f.raw.complete(request)
  await start
  const stop = f.session.stopScan()
  f.raw.complete(undefined, { error: 7, backendError: -121 })
  await assert.rejects(stop, { code: "BACKEND", backendError: -121 })
  f.raw.feature.request = () => -3
  await assert.rejects(f.session.stopScan(), { code: "BUSY" })
  f.raw.feature.request = () => { throw new Error("native throw") }
  await assert.rejects(f.session.stopScan(), /native throw/)
})

test("advertising sends exact binary view bytes and rejects oversize payloads before native", async () => {
  const f = setup()
  const array = new Uint8Array([1, 161, 2])
  const start = f.session.startAdvertising({ data: array.subarray(1, 2) })
  const request = f.raw.requests.at(-1)
  assert.equal(request.op, OP.START_ADVERTISING)
  assert.deepEqual([...new Uint8Array(request.data)], [161])
  array[1] = 9
  assert.deepEqual([...new Uint8Array(request.data)], [161])
  f.raw.complete()
  await start
  await assert.rejects(f.session.startAdvertising(), { code: "BUSY" })
  const stop = f.session.stopAdvertising()
  f.raw.complete()
  await stop
  const count = f.raw.requests.length
  await assert.rejects(f.session.startAdvertising({ data: new Uint8Array(9) }), { code: "INVALID_ARGUMENT" })
  assert.equal(f.raw.requests.length, count)
  await close(f)
})

test("outbound connection resolves one Link and buffers same-drain data before Promise handlers", async () => {
  const f = setup()
  let inbound = 0
  f.session.onConnection(() => inbound++)
  const pending = f.session.connect(incoming.peerId)
  assert.equal(f.session.connect(incoming.peerId), pending)
  const request = f.raw.requests.at(-1)
  assert.equal(request.op, OP.CONNECT)
  assert.equal(request.peerId, incoming.peerId)
  let connected = false
  void pending.then(() => { connected = true })
  f.raw.emit({ ...incoming, token: request.token })
  await turn()
  assert.equal(connected, false)
  f.raw.complete(request, { ...incoming, kind: EVENT.COMPLETE })
  f.raw.emit({ kind: EVENT.DATA, link: 42, data: new Uint8Array([0, 128, 255]).buffer })
  const link = await pending
  assert.equal(await f.session.connect(incoming.peerId), link)
  assert.equal(inbound, 0)
  assert.equal(link.id, "42")
  assert.equal(link.maxMessageBytes, 512)
  const messages = []
  const unsubscribe = link.onData(data => messages.push([...new Uint8Array(data)]))
  await turn()
  assert.deepEqual(messages, [[0, 128, 255]])
  unsubscribe()
  unsubscribe()
  f.raw.emit({ ...incoming, token: request.token })
  assert.equal(inbound, 0)
  await close(f)
})

test("inbound and outbound Links use identical send/data/disconnect APIs and session isolation", async () => {
  const f = setup()
  let link
  f.session.onConnection(value => { link = value })
  f.raw.emit(incoming)
  const messages = []
  link.onData(value => messages.push(value.byteLength))
  const sending = link.send(new Uint8Array([9, 0, 255]))
  assert.equal(f.raw.requests.at(-1).op, OP.SEND)
  assert.equal(f.raw.requests.at(-1).handle, 42)
  f.raw.complete()
  await sending
  f.raw.emit({ kind: EVENT.DATA, session: 12, link: 42, data: new Uint8Array(9).buffer })
  f.raw.emit({ kind: EVENT.DATA, link: 42, data: new Uint8Array(3).buffer })
  assert.deepEqual(messages, [3])
  await assert.rejects(link.send(new Uint8Array(513)), { code: "INVALID_ARGUMENT" })
  const reasons = []
  const unsubscribe = link.onDisconnect(reason => reasons.push(reason))
  const disconnect = link.disconnect()
  f.raw.complete()
  await disconnect
  await link.disconnect()
  unsubscribe()
  assert.deepEqual(reasons, ["DISCONNECTED"])
  await assert.rejects(link.send(new Uint8Array()), { code: "DISCONNECTED" })
  await close(f)
})

test("disconnect rejects in-flight sends but only COMPLETE settles its request", async () => {
  const f = setup()
  let link
  f.session.onConnection(value => { link = value })
  f.raw.emit(incoming)
  const sending = link.send(new Uint8Array(1))
  const rejected = assert.rejects(sending, { code: "RECEIVE_OVERFLOW", backendError: 9 })
  const disconnect = link.disconnect()
  const request = f.raw.requests.at(-1)
  let completed = false
  void disconnect.then(() => { completed = true })
  f.raw.emit({ kind: EVENT.DISCONNECT, link: 42, token: 0, error: 6, backendError: 9 })
  await rejected
  await turn()
  assert.equal(completed, false)
  f.raw.complete(request)
  await disconnect
  await close(f)
})

test("early SDU queue is bounded, overflows visibly and requests native disconnect", async () => {
  const f = setup()
  let link
  f.session.onConnection(value => { link = value })
  f.raw.emit(incoming)
  for (let i = 0; i < 5; i++) f.raw.emit({ kind: EVENT.DATA, link: 42, data: new Uint8Array(512).buffer })
  assert.equal(f.raw.requests.at(-1).op, OP.DISCONNECT)
  let reason
  link.onDisconnect(value => { reason = value })
  await turn()
  assert.equal(reason, "RECEIVE_OVERFLOW")
  f.raw.complete()
  await turn()
  await close(f)
})

test("close rejects pending operations, removes listeners, ignores late events, and awaits native close", async () => {
  const f = setup()
  let advertisements = 0
  f.session.onAdvertisement(() => advertisements++)
  const scanning = f.session.startScan()
  const oldRequest = f.raw.requests.at(-1)
  const rejected = assert.rejects(scanning, { code: "DISCONNECTED" })
  const closing = f.session.close()
  assert.equal(f.session.close(), closing)
  f.raw.complete(oldRequest)
  f.raw.emit({ kind: EVENT.ADVERTISEMENT, peerId: "late", data: new ArrayBuffer(0) })
  assert.equal(advertisements, 0)
  await rejected
  let disposed = false
  const disposal = f.facade.dispose().then(() => { disposed = true })
  await turn()
  assert.equal(f.raw.unsubscriptions, 0)
  assert.equal(disposed, false)
  f.raw.complete()
  await closing
  await disposal
  assert.equal(f.raw.unsubscriptions, 1)
  assert.throws(() => f.facade.createSession({ serviceId: "chat" }), { code: "DISCONNECTED" })
})


test("connect completion metadata is accepted and a same-turn disconnect cannot resurrect the Link", async () => {
  const f = setup()
  const connect = f.session.connect(incoming.peerId)
  f.raw.complete(undefined, { ...incoming, kind: EVENT.COMPLETE })
  f.raw.emit({ kind: EVENT.DISCONNECT, link: 42, error: 5 })
  await assert.rejects(connect, { code: "DISCONNECTED" })
  await close(f)
})

test("advertisement callbacks receive serviceId and full data, unsubscribe is idempotent", async () => {
  const f = setup()
  const reports = []
  const unsubscribe = f.session.onAdvertisement(event => reports.push(event))
  const data = new Uint8Array([161]).buffer
  f.raw.emit({ kind: EVENT.ADVERTISEMENT, peerId: incoming.peerId, rssi: -62, data })
  assert.equal(reports[0].serviceId, "chat")
  assert.equal(reports[0].data, data)
  assert.equal(reports[0].rssi, -62)
  unsubscribe()
  unsubscribe()
  f.raw.emit({ kind: EVENT.ADVERTISEMENT, data })
  assert.equal(reports.length, 1)
  await close(f)
})
