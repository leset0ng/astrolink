import { loadAstrolink } from "@canopus/astrolink"
import type { Astrolink, Link, Session } from "@canopus/astrolink"
import { makeProbe, parseProbe, verifyResponse } from "./probe"

const STEP_TIMEOUT_MS = 15_000

export interface PeerProbeView {
  status: string
  step: string
  detail: string
  passed: number
  failed: boolean
}

/** Automatic real-peer CoC acceptance; no touch input or synthetic transport. */
export class NativePeerRunner {
  private readonly view: PeerProbeView = {
    status: "双向 CoC 探针",
    step: "等待启动",
    detail: "扫描、连接、双向消息与清理",
    passed: 0,
    failed: false
  }
  private session: Session | undefined
  private link: Link | undefined
  private destroyed = false

  constructor(private readonly update: (view: PeerProbeView) => void) {}

  async start(): Promise<void> {
    let astrolink: Astrolink | undefined
    try {
      astrolink = await this.step("加载原生接口", () => loadAstrolink())
      this.session = await this.step("创建 chat Session", () => astrolink!.createSession({ serviceId: "chat" }))
      const session = this.session
      let peerId = ""
      let peerReady: (() => void) | undefined
      const peerFound = new Promise<void>(resolve => { peerReady = resolve })
      const unsubscribe = session.onAdvertisement(event => {
        if (!peerId) {
          peerId = event.peerId
          peerReady?.()
        }
      })
      try {
        await this.step("开始扫描", () => session.startScan())
        await this.step("发现真实 peripheral", () => peerFound)
        await this.step("停止扫描", () => session.stopScan())
      } finally {
        unsubscribe()
        await session.stopScan().catch(() => undefined)
      }
      this.link = await this.step("建立 LE CoC", () => session.connect(peerId))
      const link = this.link
      const responses = new Map<number, Uint8Array>()
      const unsubscribeData = link.onData(data => {
        const packet = parseProbe(data)
        if (packet?.response) responses.set(packet.sequence, packet.bytes)
      })
      try {
        await this.step("发送空 SDU", () => link.send(new Uint8Array()))
        await this.step("发送 1 字节 SDU", () => link.send(new Uint8Array([0xa5])))
        for (const size of [32, 511, 512]) {
          await this.step(`验证 ${size} 字节回包`, async () => {
            const sequence = 0xa66000 + size
            const request = makeProbe(sequence, size)
            await link.send(request)
            const deadline = Date.now() + STEP_TIMEOUT_MS
            while (!responses.has(sequence)) {
              if (Date.now() >= deadline) throw new Error(`未收到 ${size} 字节回包`)
              await new Promise(resolve => setTimeout(resolve, 20))
            }
            if (!verifyResponse(request, responses.get(sequence)!)) throw new Error(`${size} 字节回包内容不匹配`)
          })
        }
      } finally {
        unsubscribeData()
      }
      await this.step("断开 Link", () => link.disconnect())
      this.link = undefined
      await this.step("关闭 Session", () => session.close())
      this.session = undefined
      this.view.status = "双向 CoC 通过"
      this.view.step = `${this.view.passed} 项检查已完成`
      this.view.detail = "0/1 字节发送及 32/511/512 字节回包均通过"
      console.log(`[AstrolinkProbe] ${JSON.stringify({ event: "complete", step: this.view.step, passed: this.view.passed })}`)
    } catch (error) {
      if (this.destroyed) return
      this.view.status = "双向 CoC 失败"
      this.view.detail = String(error)
      this.view.failed = true
      console.error(`[AstrolinkProbe] ${JSON.stringify({ event: "failed", step: this.view.step, error: this.view.detail })}`)
    } finally {
      await this.cleanup()
      this.publish()
    }
  }

  async destroy(): Promise<void> {
    this.destroyed = true
    await this.cleanup()
  }

  private async cleanup(): Promise<void> {
    if (this.link) await this.link.disconnect().catch(() => undefined)
    this.link = undefined
    if (this.session) await this.session.close().catch(() => undefined)
    this.session = undefined
  }

  private async step<T>(name: string, action: () => T | Promise<T>): Promise<T> {
    if (this.destroyed) throw new Error("页面已销毁")
    this.view.step = name
    this.view.detail = "等待真实原生结果"
    this.publish()
    let timer: ReturnType<typeof setTimeout> | undefined
    try {
      const result = await new Promise<T>((resolve, reject) => {
        timer = setTimeout(() => reject(new Error(`TIMEOUT：${name}`)), STEP_TIMEOUT_MS)
        Promise.resolve().then(action).then(resolve, reject)
      })
      this.view.passed++
      console.log(`[AstrolinkProbe] ${JSON.stringify({ event: "pass", step: name, passed: this.view.passed })}`)
      this.publish()
      return result
    } finally {
      if (timer !== undefined) clearTimeout(timer)
    }
  }

  private publish(): void {
    if (!this.destroyed) this.update({ ...this.view })
  }
}
