import { loadAstrolink } from "@canopus/astrolink"
import type { Astrolink, Capabilities, Session } from "@canopus/astrolink"

const STEP_TIMEOUT_MS = 10_000
const LOG_LIMIT = 12

export interface ProbeLog {
  key: number
  message: string
  failed: boolean
}

export interface ProbeView {
  status: string
  step: string
  detail: string
  capabilities: string
  passed: number
  finished: boolean
  failed: boolean
  log: ProbeLog[]
}

/** A real native smoke run. Fixtures and alternate transports are not included. */
export class NativeProbeRunner {
  private readonly sessions: Session[] = []
  private destroyed = false
  private started = false
  private sequence = 0
  private cancelStep: (() => void) | null = null
  private readonly view: ProbeView = {
    status: "原生接口探针",
    step: "等待启动",
    detail: "每一步等待原生操作完成，超时即失败",
    capabilities: "尚未读取能力",
    passed: 0,
    finished: false,
    failed: false,
    log: []
  }

  constructor(private readonly update: (view: ProbeView) => void) {}

  async start(): Promise<void> {
    if (this.started || this.destroyed) return
    this.started = true
    this.view.status = "正在测试"
    this.publish()
    let astrolink: Astrolink | undefined
    try {
      astrolink = await this.step("加载原生 Feature", () => loadAstrolink())
      const caps = await this.step("读取设备能力", () => {
        const capabilities = astrolink!.getCapabilities()
        this.validateCapabilities(capabilities)
        this.view.capabilities = `${capabilities.maxLinks} Link · 服务名 ${capabilities.maxServiceIdBytes} B · 广播 ${capabilities.maxAdvertisingDataBytes} B`
        return capabilities
      })
      if (caps.maxServiceIdBytes < 1) throw new Error("本探针要求服务名上限至少 1 B")
      const boundary = "a".repeat(caps.maxServiceIdBytes)
      const utf8 = await this.step(`接受 ${caps.maxServiceIdBytes} 字节服务名`, () => this.track(astrolink!.createSession({ serviceId: boundary })))
      await this.step("关闭边界测试 Session", () => this.close(utf8))
      await this.step(`拒绝 ${caps.maxServiceIdBytes + 1} 字节服务名`, () => {
        let accidental: Session | undefined
        try { accidental = astrolink!.createSession({ serviceId: boundary + "a" }) }
        catch (error) {
          if ((error as { code?: string })?.code === "INVALID_ARGUMENT") return
          throw error
        }
        this.track(accidental)
        throw new Error(`${caps.maxServiceIdBytes + 1} B 服务名未被拒绝`)
      })

      let session = await this.step("创建 chat Session", () => this.track(astrolink!.createSession({ serviceId: "chat" })))
      await this.step("开始扫描", () => session.startScan())
      await this.step("停止扫描", () => session.stopScan())
      await this.step("重复停止扫描", () => session.stopScan())
      await this.step("开始广播 1 字节", () => session.startAdvertising({ data: new Uint8Array([0xa5]) }))
      await this.step("停止广播", () => session.stopAdvertising())
      await this.step("重复停止广播", () => session.stopAdvertising())
      await this.step("关闭 chat Session", () => this.close(session))
      await this.step("重复关闭 Session", () => session.close())
      for (let round = 1; round <= 2; round++) {
        session = await this.step(`重建 Session ${round}/2`, () => this.track(astrolink!.createSession({ serviceId: "chat" })))
        await this.step(`释放 Session ${round}/2`, () => this.close(session))
      }
      if (this.destroyed) return
      this.view.status = "原生测试通过"
      this.view.step = `${this.view.passed} 项检查已完成`
      this.view.detail = "已验证扫描、广播、字节边界和资源释放"
      this.view.finished = true
      this.record("complete", this.view.step)
    } catch (error) {
      if (this.destroyed) return
      const message = this.errorMessage(error)
      this.view.status = "原生测试失败"
      this.view.detail = message
      this.view.failed = true
      this.view.finished = true
      this.record("failed", this.view.step, { error: message })
    } finally {
      await this.cleanup()
      this.publish()
    }
  }

  async destroy(): Promise<void> {
    if (this.destroyed) return
    this.destroyed = true
    this.cancelStep?.()
    await this.cleanup()
  }

  private validateCapabilities(caps: Capabilities): void {
    if (!caps || !caps.supported) throw new Error("UNSUPPORTED：模块尚未激活或固件不支持")
    for (const field of [caps.maxLinks, caps.maxServiceIdBytes, caps.maxAdvertisingDataBytes]) {
      if (!Number.isInteger(field) || field < 1) throw new Error("原生能力字段无效")
    }
    console.log("[AstrolinkProbe] " + JSON.stringify({ event: "capabilities", ...caps }))
  }

  private async step<T>(name: string, action: () => T | Promise<T>): Promise<T> {
    if (this.destroyed) throw new Error("页面已销毁")
    this.view.step = name
    this.view.detail = "等待原生结果 · 最长 10 秒"
    this.record("start", name)
    this.publish()
    const started = Date.now()
    let timer: ReturnType<typeof setTimeout> | undefined
    try {
      const result = await new Promise<T>((resolve, reject) => {
        this.cancelStep = () => reject(new Error("页面已销毁"))
        timer = setTimeout(() => reject(new Error(`TIMEOUT：${name}超过 10 秒`)), STEP_TIMEOUT_MS)
        Promise.resolve().then(() => {
          if (this.destroyed) throw new Error("页面已销毁")
          return action()
        }).then(resolve, reject)
      })
      if (this.destroyed) throw new Error("页面已销毁")
      this.view.passed++
      this.record("pass", name, { elapsedMs: Date.now() - started })
      this.publish()
      return result
    } finally {
      if (timer !== undefined) clearTimeout(timer)
      this.cancelStep = null
    }
  }

  private track(session: Session): Session {
    if (!session || typeof session.close !== "function") throw new Error("原生 Session 无效")
    this.sessions.push(session)
    return session
  }

  private async close(session: Session): Promise<void> {
    await session.close()
    const index = this.sessions.indexOf(session)
    if (index >= 0) this.sessions.splice(index, 1)
  }

  private async cleanup(): Promise<void> {
    const sessions = this.sessions.splice(0)
    await Promise.all(sessions.map(async session => {
      let timer: ReturnType<typeof setTimeout> | undefined
      try {
        await Promise.race([
          Promise.resolve().then(() => session.close()),
          new Promise<never>((_, reject) => {
            timer = setTimeout(() => reject(new Error("TIMEOUT：清理 Session 超过 10 秒")), STEP_TIMEOUT_MS)
          })
        ])
        console.log("[AstrolinkProbe] " + JSON.stringify({ event: "cleanup", result: "pass" }))
      } catch (error) {
        const message = this.errorMessage(error)
        console.error("[AstrolinkProbe] " + JSON.stringify({ event: "cleanup", result: "failed", error: message }))
        if (!this.destroyed) {
          this.view.status = "原生测试失败"
          this.view.detail = message
          this.view.failed = true
          this.view.finished = true
          this.record("failed", "清理 Session", { error: message })
        }
      } finally {
        if (timer !== undefined) clearTimeout(timer)
      }
    }))
  }

  private record(event: string, step: string, extra: Record<string, unknown> = {}): void {
    const sequence = ++this.sequence
    console.log("[AstrolinkProbe] " + JSON.stringify({ sequence, event, step, passed: this.view.passed, ...extra }))
    if (event === "start") return
    this.view.log = this.view.log.concat({ key: sequence, message: `${event === "pass" ? "通过" : event === "complete" ? "完成" : "失败"} · ${step}`, failed: event === "failed" }).slice(-LOG_LIMIT)
  }

  private errorMessage(error: unknown): string {
    const value = error as { code?: string; message?: string; backendError?: number }
    const message = value?.message || String(error)
    return `${value?.code && !message.includes(value.code) ? value.code + "：" : ""}${message}${value?.backendError ? ` (${value.backendError})` : ""}`
  }

  private publish(): void {
    if (!this.destroyed) this.update({ ...this.view, log: this.view.log.slice() })
  }
}
