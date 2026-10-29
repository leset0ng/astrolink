// Actual numeric Feature ABI fixture. This file is excluded from the RPK.
function primitive() {
  let callback
  let nextSession = 10
  let subscriptions = 0
  let unsubscriptions = 0
  const requests = []
  const sessions = new Map()
  const feature = {
    getCapabilities: () => ({ supported: true, maxLinks: 1, maxServiceIdBytes: 6, maxAdvertisingDataBytes: 1 }),
    createSession(serviceId) {
      if (typeof serviceId !== "string" || !serviceId || Buffer.byteLength(serviceId) > 6) return -2
      const handle = ++nextSession
      sessions.set(handle, serviceId)
      return handle
    },
    request(op, handle, token, data, peerId, flag) {
      requests.push({ op, handle, token, data, peerId, flag })
      return 0
    },
    subscribe(value) {
      callback = value
      if (value) subscriptions++
      else unsubscriptions++
      return 0
    }
  }
  function emit(event) {
    callback?.({ kind: 1, token: 0, session: 11, link: 0, peerId: "", maxMessageBytes: 0,
      rssi: 0, error: 0, backendError: 0, data: new ArrayBuffer(0), ...event })
  }
  function complete(request = requests.at(-1), event = {}) {
    emit({ kind: 1, token: request.token, session: request.op === 6 || request.op === 7 ? 11 : request.handle, ...event })
  }
  return { feature, requests, sessions, emit, complete,
    get subscriptions() { return subscriptions }, get unsubscriptions() { return unsubscriptions } }
}
module.exports = { primitive }
