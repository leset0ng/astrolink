import type { Advertisement, Astrolink, Capabilities, Link, Session, Unsubscribe } from "./contract.js"
import { utf8ByteLength } from "./utf8.js"

/** Matches src/qjs/mod.rs. This is the firmware Feature ABI, not the public API. */
export const OP = { START_SCAN: 1, STOP_SCAN: 2, START_ADVERTISING: 3, STOP_ADVERTISING: 4,
  CONNECT: 5, SEND: 6, DISCONNECT: 7, CLOSE: 8 } as const
export const EVENT = { COMPLETE: 1, ADVERTISEMENT: 2, CONNECTION: 3, DATA: 4, DISCONNECT: 5 } as const
export interface NativeEvent {
  kind: number
  token: number
  session: number
  link: number
  peerId: string
  maxMessageBytes: number
  rssi: number
  error: number
  backendError: number
  data: ArrayBuffer
}
export interface PrimitiveFeature {
  getCapabilities(): Capabilities
  createSession(serviceId: string): number
  request(operation: number, handle: number, token: number, data?: ArrayBuffer,
    peerId?: string, flag?: boolean): number
  subscribe(callback?: (event: NativeEvent) => void): number
}

const ERROR_CODES = ["BACKEND", "UNSUPPORTED", "INVALID_ARGUMENT", "BUSY", "TIMEOUT", "DISCONNECTED", "RECEIVE_OVERFLOW", "BACKEND"]
const MAX_PENDING_REQUESTS = 32
const MAX_EARLY_MESSAGES = 4
const EMPTY = new ArrayBuffer(0)

export class AstrolinkError extends Error {
  readonly code: string
  readonly nativeCode: number
  readonly backendError: number
  constructor(error: number, backendError = 0, message?: string) {
    const nativeCode = Math.abs(error)
    const code = ERROR_CODES[nativeCode] || "BACKEND"
    super(message || `${code}${backendError ? ` (${backendError})` : ""}`)
    this.name = "AstrolinkError"
    this.code = code
    this.nativeCode = nativeCode
    this.backendError = backendError
  }
}

function invalid(message: string): AstrolinkError { return new AstrolinkError(2, 0, message) }
function disconnected(): AstrolinkError { return new AstrolinkError(5) }
function binary(data: ArrayBuffer | Uint8Array): ArrayBuffer {
  if (data instanceof ArrayBuffer) return data.slice(0)
  if (data instanceof Uint8Array) return data.slice().buffer as ArrayBuffer
  throw invalid("数据必须是 ArrayBuffer 或 Uint8Array")
}
function listen<T>(listeners: Array<(value: T) => void>, callback: (value: T) => void): Unsubscribe {
  if (typeof callback !== "function") throw invalid("回调必须是函数")
  listeners.push(callback)
  let active = true
  return () => {
    if (!active) return
    active = false
    const index = listeners.indexOf(callback)
    if (index >= 0) listeners.splice(index, 1)
  }
}
function emit<T>(listeners: Array<(value: T) => void>, value: T): void {
  for (const callback of listeners.slice()) {
    // Unsubscribing another callback during dispatch takes effect immediately.
    if (!listeners.includes(callback)) continue
    try { callback(value) } catch (error) { console.error("[Astrolink] callback failed", error) }
  }
}
interface Pending {
  operation: number
  session: SessionFacade
  link?: LinkFacade
  resolve: (event: NativeEvent) => void
  reject: (error: Error) => void
}

export class AstrolinkFacade implements Astrolink {
  private readonly sessions = new Map<number, SessionFacade>()
  private readonly pending = new Map<number, Pending>()
  private token = 0
  private disposing = false
  private disposal: Promise<void> | null = null

  constructor(private readonly native: PrimitiveFeature) {
    const result = native.subscribe(event => this.onEvent(event))
    if (result < 0) throw new AstrolinkError(result)
  }

  getCapabilities(): Capabilities { return { ...this.native.getCapabilities() } }

  createSession(options: { serviceId: string }): Session {
    if (this.disposing) throw disconnected()
    const serviceId = options?.serviceId
    if (typeof serviceId !== "string" || !serviceId ||
        utf8ByteLength(serviceId) > this.getCapabilities().maxServiceIdBytes) throw invalid("serviceId 超出 UTF-8 字节上限")
    const handle = this.native.createSession(serviceId)
    if (handle < 0) throw new AstrolinkError(handle)
    if (!Number.isInteger(handle) || handle <= 0 || this.sessions.has(handle)) throw new AstrolinkError(7)
    const session = new SessionFacade(this, handle, serviceId)
    this.sessions.set(handle, session)
    return session
  }

  request(operation: number, session: SessionFacade, link?: LinkFacade,
    data = EMPTY, peerId?: string, flag = false): Promise<NativeEvent> {
    if (this.pending.size >= MAX_PENDING_REQUESTS || this.token >= 0x7fffffff) return Promise.reject(new AstrolinkError(3))
    // Allocate before calling native: even a synchronous test callback cannot race registration.
    const token = ++this.token
    return new Promise((resolve, reject) => {
      this.pending.set(token, { operation, session, link, resolve, reject })
      try {
        const result = this.native.request(operation, link?.handle || session.handle, token, data, peerId, flag)
        if (result < 0) {
          this.pending.delete(token)
          reject(new AstrolinkError(result))
        }
      } catch (error) {
        this.pending.delete(token)
        reject(error instanceof Error ? error : new AstrolinkError(7, 0, String(error)))
      }
    })
  }

  cancelSession(session: SessionFacade): void {
    for (const [token, pending] of this.pending) {
      if (pending.session === session && pending.operation !== OP.CLOSE) {
        this.pending.delete(token)
        pending.reject(disconnected())
      }
    }
  }

  cancelLink(link: LinkFacade, error: AstrolinkError): void {
    for (const [token, pending] of this.pending) {
      if (pending.link === link && pending.operation !== OP.DISCONNECT) {
        this.pending.delete(token)
        pending.reject(error)
      }
    }
  }

  removeSession(session: SessionFacade): void {
    if (this.sessions.get(session.handle) === session) this.sessions.delete(session.handle)
  }

  dispose(): Promise<void> {
    if (this.disposal) return this.disposal
    this.disposing = true
    this.disposal = Promise.all(Array.from(this.sessions.values(), session =>
      session.close().catch(error => console.error("[Astrolink] Session close failed", error)))).then(() => {
      // Keep the subscription until every close token has completed.
      const result = this.native.subscribe(undefined)
      for (const pending of this.pending.values()) pending.reject(disconnected())
      this.pending.clear()
      this.sessions.clear()
      if (result < 0) throw new AstrolinkError(result)
    })
    return this.disposal
  }

  private onEvent(event: NativeEvent): void {
    const session = this.sessions.get(event.session)
    if (!session) return
    const pending = event.token ? this.pending.get(event.token) : undefined
    if (pending && pending.session !== session) return
    if (event.kind === EVENT.COMPLETE) {
      if (!pending) return
      this.pending.delete(event.token)
      if (event.error) pending.reject(new AstrolinkError(event.error, event.backendError))
      else {
        try {
          // Install before resolving: later DATA in this native drain must queue.
          if (pending.operation === OP.CONNECT) session.prepareLink(event)
          pending.resolve(event)
        } catch (error) {
          pending.reject(error instanceof Error ? error : new AstrolinkError(7))
        }
      }
      return
    }
    if (session.closing) return
    if (event.kind === EVENT.CONNECTION) {
      // Frozen native contract: CONNECTION is inbound only, always token zero.
      if (event.token === 0 && !event.error) session.incoming(event)
    } else if (event.kind === EVENT.ADVERTISEMENT) {
      session.advertisement(event)
    } else if (event.kind === EVENT.DATA) {
      session.data(event)
    } else if (event.kind === EVENT.DISCONNECT) {
      session.disconnected(event)

    }
  }
}

class SessionFacade implements Session {
  readonly links = new Map<number, LinkFacade>()
  private readonly advertisements: Array<(event: Advertisement) => void> = []
  private readonly connections: Array<(link: Link) => void> = []
  private readonly connecting = new Map<string, Promise<Link>>()
  private closePromise: Promise<void> | null = null
  closing = false
  private scanning = false
  private advertisingActive = false
  private scanStarting = false
  private advertisementStarting = false

  constructor(private readonly owner: AstrolinkFacade, readonly handle: number, private readonly serviceId: string) {}

  onAdvertisement(callback: (event: Advertisement) => void): Unsubscribe { return listen(this.advertisements, callback) }
  onConnection(callback: (link: Link) => void): Unsubscribe { return listen(this.connections, callback) }

  async startScan(options?: { allowDuplicates?: boolean }): Promise<void> {
    this.open()
    if (this.scanning || this.scanStarting) throw new AstrolinkError(3)
    this.scanStarting = true
    try {
      await this.owner.request(OP.START_SCAN, this, undefined, EMPTY, undefined, options?.allowDuplicates === true)
      if (!this.closing) this.scanning = true
    } finally { this.scanStarting = false }
  }
  async stopScan(): Promise<void> {
    if (this.closing) return
    await this.owner.request(OP.STOP_SCAN, this)
    this.scanning = false
  }
  async startAdvertising(options?: { data?: ArrayBuffer | Uint8Array }): Promise<void> {
    this.open()
    if (this.advertisingActive || this.advertisementStarting) throw new AstrolinkError(3)
    const data = options?.data === undefined ? EMPTY : binary(options.data)
    if (data.byteLength > this.owner.getCapabilities().maxAdvertisingDataBytes) throw invalid("广播数据超出字节上限")
    this.advertisementStarting = true
    try {
      await this.owner.request(OP.START_ADVERTISING, this, undefined, data)
      if (!this.closing) this.advertisingActive = true
    } finally { this.advertisementStarting = false }
  }
  async stopAdvertising(): Promise<void> {
    if (this.closing) return
    await this.owner.request(OP.STOP_ADVERTISING, this)
    this.advertisingActive = false
  }
  connect(peerId: string): Promise<Link> {
    if (this.closing) return Promise.reject(disconnected())
    if (typeof peerId !== "string" || !peerId) return Promise.reject(invalid("peerId 不能为空"))
    for (const link of this.links.values()) if (link.peerId === peerId && !link.closed) return Promise.resolve(link)
    const existing = this.connecting.get(peerId)
    if (existing) return existing
    const promise = this.owner.request(OP.CONNECT, this, undefined, EMPTY, peerId).then(event => {
      if (this.closing) throw disconnected()
      const link = this.links.get(event.link)
      if (!link || link.closed) throw disconnected()
      return link
    }).finally(() => this.connecting.delete(peerId))
    this.connecting.set(peerId, promise)
    return promise
  }
  close(): Promise<void> {
    if (this.closePromise) return this.closePromise
    this.closing = true
    this.owner.cancelSession(this)
    this.advertisements.length = 0
    this.connections.length = 0
    for (const link of this.links.values()) link.finish(disconnected())
    this.links.clear()
    this.connecting.clear()
    this.closePromise = this.owner.request(OP.CLOSE, this).then(() => {}, error => { throw error }).finally(() => {
      this.owner.removeSession(this)
    })
    return this.closePromise
  }
  prepareLink(event: NativeEvent): void { this.getLink(event) }
  incoming(event: NativeEvent): void {
    emit(this.connections, this.getLink(event))
  }
  advertisement(event: NativeEvent): void {
    emit(this.advertisements, { peerId: event.peerId, serviceId: this.serviceId, data: event.data, rssi: event.rssi })
  }
  data(event: NativeEvent): void { this.links.get(event.link)?.receive(event.data) }
  disconnected(event: NativeEvent): void {
    const link = this.links.get(event.link)
    if (!link) return
    link.finish(new AstrolinkError(event.error || 5, event.backendError))
    this.links.delete(event.link)
  }
  private open(): void { if (this.closing) throw disconnected() }
  private getLink(event: NativeEvent): LinkFacade {
    const existing = this.links.get(event.link)
    if (existing) return existing
    if (!event.link || !event.peerId || event.maxMessageBytes <= 0) throw new AstrolinkError(7, 0, "原生连接事件缺少 Link 元数据")
    const link = new LinkFacade(this.owner, this, event.link, event.peerId, event.maxMessageBytes)
    this.links.set(event.link, link)
    return link
  }
}

class LinkFacade implements Link {
  readonly id: string
  private readonly dataListeners: Array<(data: ArrayBuffer) => void> = []
  private readonly disconnectListeners: Array<(reason: string) => void> = []
  private readonly earlyMessages: ArrayBuffer[] = []
  private disconnectPromise: Promise<void> | null = null
  private reason: AstrolinkError | null = null
  closed = false

  constructor(private readonly owner: AstrolinkFacade, private readonly session: SessionFacade,
    readonly handle: number, readonly peerId: string, readonly maxMessageBytes: number) {
    this.id = String(handle)
  }
  onData(callback: (data: ArrayBuffer) => void): Unsubscribe {
    const unsubscribe = listen(this.dataListeners, callback)
    if (this.earlyMessages.length) {
      // Registration returns before user callbacks; queued SDUs retain their ordering.
      void Promise.resolve().then(() => {
        while (!this.closed && this.earlyMessages.length && this.dataListeners.length) {
          emit(this.dataListeners, this.earlyMessages.shift()!)
        }
      })
    }
    return unsubscribe
  }
  onDisconnect(callback: (reason: string) => void): Unsubscribe {
    const unsubscribe = listen(this.disconnectListeners, callback)
    if (this.reason) void Promise.resolve().then(() => {
      if (this.disconnectListeners.includes(callback)) {
        try { callback(this.reason!.code) } catch (error) { console.error("[Astrolink] callback failed", error) }
      }
    })
    return unsubscribe
  }
  async send(data: ArrayBuffer | Uint8Array): Promise<void> {
    if (this.closed || this.disconnectPromise || this.session.closing) throw disconnected()
    const buffer = binary(data)
    if (buffer.byteLength > this.maxMessageBytes) throw invalid("消息超出 maxMessageBytes")
    await this.owner.request(OP.SEND, this.session, this, buffer)
  }
  disconnect(): Promise<void> {
    if (this.disconnectPromise) return this.disconnectPromise
    if (this.closed || this.session.closing) return Promise.resolve()
    this.disconnectPromise = this.owner.request(OP.DISCONNECT, this.session, this).then(() => {
      this.finish(disconnected())
      this.session.links.delete(this.handle)
    }).catch(error => { this.disconnectPromise = null; throw error })
    return this.disconnectPromise
  }
  receive(data: ArrayBuffer): void {
    if (this.closed) return
    if (this.dataListeners.length && !this.earlyMessages.length) emit(this.dataListeners, data)
    else if (this.earlyMessages.length < MAX_EARLY_MESSAGES) this.earlyMessages.push(data)
    else {
      const closing = this.disconnect()
      this.finish(new AstrolinkError(6))
      void closing.catch(error => console.error("[Astrolink] overflow disconnect failed", error))
    }
  }
  finish(error: AstrolinkError): void {
    if (this.closed) return
    this.closed = true
    this.reason = error
    this.owner.cancelLink(this, error)
    this.earlyMessages.length = 0
    this.dataListeners.length = 0
    emit(this.disconnectListeners, error.code)
    this.disconnectListeners.length = 0
  }
}
