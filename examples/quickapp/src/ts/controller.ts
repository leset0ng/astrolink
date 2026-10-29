import type { Advertisement, Astrolink, Capabilities, Link, Session, Unsubscribe } from "@canopus/astrolink"
import { makeProbe, makeResponse, parseProbe, verifyResponse } from "./probe"
import { utf8ByteLength } from "@canopus/astrolink"

const SERVICE_ID = "chat"
const MAX_PEERS = 12
const MAX_LOG = 20
const RESPONSE_TIMEOUT_MS = 8000

export interface PeerView { peerId: string; rssi: number; dataHex: string }
export interface LogView { id: number; message: string; failed: boolean }
export interface ViewState {
  supported: boolean
  ready: boolean
  status: string
  busy: string
  scanning: boolean
  advertising: boolean
  connected: boolean
  peerId: string
  maxMessageBytes: number
  sent: number
  received: number
  result: string
  peers: PeerView[]
  log: LogView[]
}
interface PendingProbe {
  expected: Uint8Array
  timer: ReturnType<typeof setTimeout>
  resolve: () => void
  reject: (error: Error) => void
}

export function describeError(error: unknown): string {
  const value = error as { code?: string | number; message?: string }
  if (value && value.code) return `${value.code}${value.message ? `: ${value.message}` : ""}`
  return String(value && value.message || error || "未知错误")
}

export class TestController {
  private session: Session | null = null
  private link: Link | null = null
  private sessionSubscriptions: Unsubscribe[] = []
  private linkSubscriptions: Unsubscribe[] = []
  private listeners: Array<(view: ViewState) => void> = []
  private pending: Record<number, PendingProbe> = {}
  private capabilities: Capabilities | null = null
  private sequence = 0
  private logId = 0
  private generation = 0
  private destroyed = false
  private operation: Promise<void> | null = null
  private closing: Promise<void> | null = null
  private view: ViewState = {
    supported: false, ready: false, status: "正在检测模块", busy: "", scanning: false,
    advertising: false, connected: false, peerId: "", maxMessageBytes: 0,
    sent: 0, received: 0, result: "两台设备打开本应用", peers: [], log: []
  }

  constructor(private readonly loadNative: () => Astrolink) {}

  subscribe(callback: (view: ViewState) => void): Unsubscribe {
    this.listeners.push(callback)
    callback(this.snapshot())
    return () => { this.listeners = this.listeners.filter(item => item !== callback) }
  }

  snapshot(): ViewState {
    return { ...this.view, peers: this.view.peers.map(peer => ({ ...peer })), log: this.view.log.slice() }
  }

  initialize(): void {
    if (this.destroyed || this.session) return
    try {
      const native = this.loadNative()
      this.capabilities = native.getCapabilities()
      this.view.supported = this.capabilities.supported
      if (!this.capabilities.supported) {
        this.view.status = "当前固件不支持"
        this.view.result = "请安装与固件匹配的 Astrolink 模块"
        this.publish()
        return
      }
      if (utf8ByteLength(SERVICE_ID) > this.capabilities.maxServiceIdBytes) {
        throw new Error("chat 的 UTF-8 字节数超过当前固件上限")
      }
      const session = native.createSession({ serviceId: SERVICE_ID })
      this.session = session
      const generation = ++this.generation
      this.sessionSubscriptions = [
        session.onAdvertisement(event => {
          if (this.current(generation)) this.onAdvertisement(event)
        }),
        session.onConnection(link => {
          if (this.current(generation)) this.attachLink(link)
        })
      ]
      this.view.ready = true
      this.view.status = "未连接 · chat"
      this.view.result = "一台开启广播，另一台扫描连接"
      this.record("Session 已创建 · chat")
    } catch (error) {
      this.view.status = "模块不可用"
      this.view.result = "安装模块并重启后重试"
      this.record(describeError(error), true)
    }
    this.publish()
  }

  toggleScan(): Promise<void> {
    return this.run("扫描", async generation => {
      const session = this.requireSession()
      if (this.view.scanning) {
        await session.stopScan()
        if (!this.current(generation)) return
        this.view.scanning = false
        this.record("扫描已停止")
      } else {
        this.view.peers = []
        await session.startScan()
        if (!this.current(generation)) return
        this.view.scanning = true
        this.record("扫描已开始")
      }
    })
  }

  toggleAdvertising(): Promise<void> {
    return this.run("广播", async generation => {
      const session = this.requireSession()
      if (this.view.advertising) {
        await session.stopAdvertising()
        if (!this.current(generation)) return
        this.view.advertising = false
        this.record("广播已停止 · 已有连接继续使用")
      } else {
        // A single byte fits every nonzero advertised data capacity.
        const size = Math.min(this.capabilities?.maxAdvertisingDataBytes || 0, 1)
        await session.startAdvertising({ data: new Uint8Array(size).fill(0xa1) })
        if (!this.current(generation)) return
        this.view.advertising = true
        this.record("广播已开始 · 等待另一台设备")
      }
    })
  }

  connect(peerId: string): Promise<void> {
    return this.run("连接", async generation => {
      if (this.link) throw new Error("请先断开当前连接")
      const link = await this.requireSession().connect(peerId)
      if (!this.current(generation)) return
      this.attachLink(link)
    })
  }

  ping(): Promise<void> {
    return this.run("Ping", async () => {
      const link = this.requireLink()
      const started = Date.now()
      await this.probe(link, Math.min(16, link.maxMessageBytes))
      this.view.result = `Ping 回包通过 · ${Date.now() - started} ms`
      this.record(this.view.result)
    })
  }

  testBinaryAndLimits(): Promise<void> {
    return this.run("边界测试", async generation => {
      const link = this.requireLink()
      await this.probe(link, Math.min(32, link.maxMessageBytes))
      if (!this.current(generation)) return
      this.record("二进制逐字节校验通过")
      await this.probe(link, link.maxMessageBytes)
      if (!this.current(generation)) return
      this.record(`${link.maxMessageBytes} B 最大消息回包通过`)
      // These are raw SDUs; the external peer records receipt independently.
      // Local submission is deliberately not reported as an application echo.
      await link.send(new Uint8Array(0))
      if (!this.current(generation)) return
      this.view.sent++
      await link.send(new Uint8Array([0xa5]))
      if (!this.current(generation)) return
      this.view.sent++
      this.record("0 B / 1 B 原始消息已提交")
      let rejected = false
      try {
        await link.send(new Uint8Array(link.maxMessageBytes + 1))
      } catch (error) {
        if (String((error as { code?: string })?.code) !== "INVALID_ARGUMENT") throw error
        rejected = true
      }
      if (!this.current(generation)) return
      if (!rejected) throw new Error("超长消息未被拒绝")
      this.view.result = "二进制、最大长度、超长拒绝均通过"
      this.record(this.view.result)
    })
  }

  testServiceNames(): Promise<void> {
    return this.run("服务名测试", async generation => {
      const native = this.loadNative()
      const maxBytes = this.capabilities?.maxServiceIdBytes || 0
      if (maxBytes < 1) throw new Error("服务名字节上限无效")
      // Two CJK characters occupy 6 UTF-8 bytes, despite string.length being 2.
      const valid = "中".repeat(Math.floor(maxBytes / 3)) + "a".repeat(maxBytes % 3)
      const invalid = valid + "a"
      const temporary = native.createSession({ serviceId: valid })
      await temporary.close()
      if (!this.current(generation)) return
      let rejected = false
      try {
        const unexpected = native.createSession({ serviceId: invalid })
        await unexpected.close()
      } catch (error) {
        if (String((error as { code?: string })?.code) !== "INVALID_ARGUMENT") throw error
        rejected = true
      }
      if (!this.current(generation)) return
      if (!rejected) throw new Error("超长 UTF-8 服务名未被拒绝")
      this.view.result = `服务名 ${utf8ByteLength(valid)} B 通过，${utf8ByteLength(invalid)} B 已拒绝`
      this.record(this.view.result)
    })
  }

  disconnect(): Promise<void> {
    return this.run("断开", async generation => {
      const link = this.link
      if (!link) return
      await link.disconnect()
      if (this.current(generation) && this.link === link) this.releaseLink("主动断开")
    })
  }

  closeSession(): Promise<void> {
    return this.close(false)
  }

  reopen(): void {
    if (!this.closing && !this.operation) this.initialize()
  }

  destroy(): Promise<void> {
    if (this.destroyed) return this.closing || Promise.resolve()
    this.destroyed = true
    this.listeners = []
    return this.close(true)
  }

  private async close(destroy: boolean): Promise<void> {
    if (this.closing) return this.closing
    if (!this.session) return
    // Invalidate callbacks before awaiting native cleanup, including in-flight starts.
    ++this.generation
    const session = this.session
    this.session = null
    for (const unsubscribe of this.sessionSubscriptions.splice(0)) unsubscribe()
    this.releaseLink("Session 已关闭")
    this.view.ready = false
    this.view.scanning = false
    this.view.advertising = false
    this.view.busy = "关闭"
    this.view.status = "正在关闭 Session"
    this.publish()
    this.closing = Promise.resolve().then(async () => {
      try {
        await session.close()
        this.view.status = "Session 已关闭"
        this.view.result = "扫描、广播与连接已释放"
        if (!destroy) this.record(this.view.result)
      } catch (error) {
        this.view.status = "Session 关闭失败"
        this.view.result = describeError(error)
        if (!destroy) this.record(this.view.result, true)
      } finally {
        this.view.busy = ""
        this.closing = null
        this.publish()
      }
    })
    return this.closing
  }

  private requireSession(): Session {
    if (!this.session) throw new Error("Session 未开启")
    return this.session
  }

  private requireLink(): Link {
    if (!this.link) throw new Error("请先连接设备")
    return this.link
  }

  private current(generation: number): boolean {
    return !this.destroyed && generation === this.generation && this.session !== null
  }

  private async run(label: string, action: (generation: number) => Promise<void>): Promise<void> {
    if (this.destroyed || this.operation || this.closing || !this.session) return
    const generation = this.generation
    this.view.busy = label
    this.publish()
    this.operation = (async () => {
      try { await action(generation) }
      catch (error) {
        if (this.current(generation)) {
          this.view.result = describeError(error)
          this.record(`${label}失败 · ${this.view.result}`, true)
        }
      } finally {
        this.operation = null
        if (this.current(generation)) {
          this.view.busy = ""
          this.publish()
        }
      }
    })()
    return this.operation
  }

  private onAdvertisement(event: Advertisement): void {
    const peers = this.view.peers.filter(peer => peer.peerId !== event.peerId)
    const data = new Uint8Array(event.data)
    const dataHex = Array.from(data.slice(0, 8), byte => byte.toString(16).padStart(2, "0")).join(" ")
    peers.unshift({ peerId: event.peerId, rssi: event.rssi, dataHex: dataHex || "无附加数据" })
    this.view.peers = peers.slice(0, MAX_PEERS)
    this.publish()
  }

  private attachLink(link: Link): void {
    if (this.link?.id === link.id) return
    if (this.link) {
      // This single-link example rejects extra arrivals without touching its current link.
      void link.disconnect().catch(error => this.record(describeError(error), true))
      return
    }
    this.link = link
    const generation = this.generation
    this.linkSubscriptions = [
      link.onData(data => {
        if (this.current(generation) && this.link === link) this.onData(link, data)
      }),
      link.onDisconnect(reason => {
        if (this.current(generation) && this.link === link) this.releaseLink(reason)
      })
    ]
    this.view.connected = true
    this.view.peerId = link.peerId
    this.view.maxMessageBytes = link.maxMessageBytes
    this.view.status = "已连接 · chat"
    this.view.result = "可发送 Ping 或运行边界测试"
    this.record(`连接成功 · 上限 ${link.maxMessageBytes} B`)
    this.publish()
  }

  private onData(link: Link, data: ArrayBuffer): void {
    this.view.received++
    const packet = parseProbe(data)
    if (!packet) {
      this.record(`收到 ${data.byteLength} B 自定义消息`)
      this.publish()
      return
    }
    if (!packet.response) {
      // Only request packets are echoed, preventing echo loops between identical apps.
      if (packet.bytes.length > link.maxMessageBytes) {
        this.record(`无法回包 · 对端发送上限 ${link.maxMessageBytes} B`, true)
      } else {
        void link.send(makeResponse(packet)).then(() => {
          if (this.link !== link || this.destroyed) return
          this.view.sent++
          this.record(`已回包 ${packet.bytes.length} B`)
          this.publish()
        }).catch(error => {
          if (this.link !== link || this.destroyed) return
          this.record(`回包失败 · ${describeError(error)}`, true)
          this.publish()
        })
      }
    } else {
      const pending = this.pending[packet.sequence]
      if (pending) {
        delete this.pending[packet.sequence]
        clearTimeout(pending.timer)
        if (verifyResponse(pending.expected, packet.bytes)) pending.resolve()
        else pending.reject(new Error("二进制回包内容不一致"))
      } else this.record("收到已过期回包")
    }
    this.publish()
  }

  private async probe(link: Link, size: number): Promise<void> {
    const sequence = ++this.sequence >>> 0
    const expected = makeProbe(sequence, size)
    let cancel: (error: Error) => void = () => {}
    const response = new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        delete this.pending[sequence]
        reject(new Error("等待回包超时，请确认另一台运行本应用"))
      }, RESPONSE_TIMEOUT_MS)
      this.pending[sequence] = { expected, timer, resolve, reject }
      cancel = error => {
        clearTimeout(timer)
        delete this.pending[sequence]
        reject(error)
      }
    })
    // A disconnect can reject the waiter while native send is still pending.
    // Attach its rejection handler immediately to avoid an unhandled rejection.
    const observedResponse = response.then(() => ({ error: null as Error | null }), error => ({ error: error as Error }))
    try {
      await link.send(expected)
      this.view.sent++
      this.publish()
    } catch (error) {
      cancel(error instanceof Error ? error : new Error(describeError(error)))
    }
    const outcome = await observedResponse
    if (outcome.error) throw outcome.error
  }

  private releaseLink(reason: string): void {
    for (const unsubscribe of this.linkSubscriptions.splice(0)) unsubscribe()
    this.link = null
    for (const key of Object.keys(this.pending)) {
      const pending = this.pending[Number(key)]
      clearTimeout(pending.timer)
      pending.reject(new Error(reason || "连接已断开"))
      delete this.pending[Number(key)]
    }
    if (this.view.connected) this.record(`连接已断开 · ${reason || "DISCONNECTED"}`)
    this.view.connected = false
    this.view.peerId = ""
    this.view.maxMessageBytes = 0
    if (this.session) this.view.status = "未连接 · chat"
    this.publish()
  }

  private record(message: string, failed = false): void {
    const entry = { id: ++this.logId, message, failed }
    this.view.log = [entry].concat(this.view.log).slice(0, MAX_LOG)
    console.log(`[Astrolink] ${failed ? "FAIL " : ""}${message}`)
  }

  private publish(): void {
    if (this.destroyed) return
    const snapshot = this.snapshot()
    for (const listener of this.listeners.slice()) listener(snapshot)
  }
}
