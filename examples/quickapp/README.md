# Astrolink 示例与链路测试快应用

这是一个直接调用 Astrolink 原生 Feature 的 Vela 快应用。两台设备安装相同应用后，使用 `chat` 服务发现对方、连接、互发二进制消息并校验回包。它没有文件 IPC、网络代理或模拟传输。

## 构建

```sh
# Build the directly installable native API package first.
cd packages/astrolink
pnpm install --frozen-lockfile
pnpm test
pnpm pack:npm

# The example installs this actual .tgz, not a source directory or alias.
cd ../../examples/quickapp
pnpm install --frozen-lockfile
pnpm typecheck
pnpm test
pnpm test:bundle
pnpm build
```

`pnpm build:probe` 构建独立原生探针，输出 `dist/astrolink-probe-source.rpk`，包名为 `com.canopus.astrolink.probe`，入口为 `pages/probe`，使用现有探针页面和 runner。构建会将源码和配置复制到临时目录，链接本项目已有的 `node_modules`，只修改临时 manifest；结束后清理临时目录，不改动 `src/manifest.json`。源码示例、字节码示例和探针的上述命名 RPK 会在后续构建中保留。安装测试时使用这个产物，无需依赖手工生成的 `/tmp/astrolink-probe-standalone.rpk`。

调试包生成于 `dist/com.canopus.astrolink.example.debug.0.1.0.rpk`，同时保存源码版本 `dist/astrolink-example-source.rpk`。`pnpm build:bytecode` 生成使用 QuickJS 字节码的调试包，并另存 `dist/astrolink-example-bytecode.rpk`，因此两种构建可以同时保留。`pnpm release` 需要先在 `sign/` 配置自己的 release 签名证书；项目不包含私钥。Node.js 18+、pnpm 和构建所需的 aiot-toolkit 均由 `package.json` / `pnpm-lock.yaml` 指定。

应用包名为 `com.canopus.astrolink.example`。先在目标设备安装与其精确固件匹配的 Astrolink 模块并激活，再通过 Mirage 的应用安装功能或 AstroBox 安装 RPK。仅安装 RPK 不会安装原生模块。

示例通过 `import { loadAstrolink } from "@lesetong/astrolink"` 使用可独立安装的 npm 包。`package.json` 依赖的是 `file:../../packages/astrolink/dist/lesetong-astrolink-0.1.1.tgz`，pnpm lock 保存其完整性校验；没有直接引用或复制包源码。修改包后重新 `pnpm pack:npm`，再在示例目录执行 `pnpm update @lesetong/astrolink` 更新 tarball 完整性与安装内容。

已确认原生 Feature 名为 `canopus.astrolink`，集中在 `packages/astrolink/src/native.ts` 和示例的 `src/manifest.json`。加载使用固件的 `$app_require$("@app-module/canopus.astrolink")`，与工具链生成系统 Feature 调用所用的机制相同。

固件原始接口仅提供 `getCapabilities()`、`createSession(serviceId) -> number`、`request(op, handle, token, data, peerId, flag) -> number` 和 `subscribe(callback)`。npm 包内的 `packages/astrolink/src/facade.ts` 将其封装为本文使用的 Session/Link API：每次请求分配独立 token，由原生事件完成 Promise；即时负错误码和异步事件错误均转换为含 `code`、`nativeCode`、`backendError` 的 Error。主动连接通过 Promise 交付，`onConnection` 只通知入站连接。实现不轮询、不使用计时器轮询事件。

app/page 两个构建 bundle 共享一个全局 façade，因此每个原生 Feature 实例只有一份事件订阅。退出时先等待 Session close 完成，再 `subscribe(undefined)` 解除原生回调。每条 Link 在应用注册 `onData` 前最多暂存 4 个完整 SDU；超过上限以 `RECEIVE_OVERFLOW` 结束 Link 并请求原生断连。

## 双机操作

1. 两台设备打开 Astrolink，确认状态为“未连接 · chat”。
2. 设备 A 点击“广播”；设备 B 点击“扫描”，在“附近设备”点击 A 的 peerId。
3. 连接后，在任一端点击“发送 Ping”。本端收到对端应用回包并逐字节验证后才显示通过与往返耗时。
4. 点击“二进制与长度测试”，依次验证二进制内容、`maxMessageBytes` 大小的完整 SDU 回包，以及 `maxMessageBytes + 1` 被原生模块以 `INVALID_ARGUMENT` 拒绝。
5. “服务名字节测试”不需要 Link：createSession 会按原生 `getCapabilities().maxServiceIdBytes` 构造一个恰好到达上限的服务名，关闭临时 Session，再验证多一个 ASCII 字节后以 `INVALID_ARGUMENT` 拒绝。它按 UTF-8 字节计数，不使用 JavaScript 字符数判定。
6. 两端交换操作方向再测一遍。收到请求的应用自动回包；响应包不会再次回包。
7. 停止广播后已有连接应继续可用；“断开连接”只关闭当前 Link；“关闭 Session”释放此应用的扫描、广播和连接。“重新检测”创建新的 Session。
8. 退出应用后重新进入，确认可以重新扫描和连接。页面和应用销毁都会调用同一个全局控制器的清理；晚到事件由 generation 校验丢弃。

若两端的发送上限不同，完整最大长度回包还需要接收端的发送上限容纳原消息；不足时对端会明确记录“无法回包”。这里验证的是应用级 round trip，`link.send()` 本身仍只承诺本地提交。

当前服务名上限为 64 个 UTF-8 字节（服务名不上线，只把派生出的 128-bit UUID 写进广播），广播附加数据上限为 8 字节，消息上限为 512 字节；界面测试以原生 `getCapabilities()` 和 Link 属性报告的值为准。

界面保留最近 12 个 peer 和 20 条事件，防止长期扫描或测试持续增长内存。事件同时以 `[Astrolink]` 前缀输出到设备日志。出现错误时保留原生错误码和消息，便于核对固件日志。

## 示例消息格式

这是快应用自己的 echo 测试格式，不是 Astrolink 的链路协议。每次 `send` 仍是一条完整 SDU；原生模块不添加 ACK 或应用消息封装。

| 字节 | 含义 |
| --- | --- |
| 0..2 | ASCII `ALQ` |
| 3 | 示例协议版本 `1` |
| 4 | 请求 `1` / 响应 `2` |
| 5..8 | 小端 uint32 序号 |
| 9..末尾 | 确定性二进制模式 |

对端复制请求并将类型改成响应。本端核对长度、序号和每个字节。8 秒未收到响应时测试失败；Link 断开时立即结束等待并清除计时器。

## 模拟器实测

模块 version code 36 已在 Q66 `4.100.155` 的隔离 Mirage 环境通过 18 项原生生命周期检查，以及示例主动连接、被动接入两种真实 LE CoC 路径。标准 LE 测试对端记录了 HELLO、0/1/511/512 B 完整消息收发和无错误的 CoC/LE 断连；示例设备日志记录了空消息与 1 B 消息的消费、逐字节二进制校验、512 B 回包和超长拒绝。两种连接方向先后复用同一个 Session，随后退出示例并重新运行生命周期探针也通过。

详细结果、固件 ABI 修复及剩余实机验证项见 `../../docs/native-validation.md`，可审查的事件与产物哈希见 `../../docs/evidence/q66-155-v36.json`。这部分证明固件与控制器模型的协议接入，尚不代表两台物理手环的射频、手机/Audio 共存或长时稳定性已通过。

## 验证边界

`pnpm test:bundle` 还会在独立 JavaScript realm 中运行工具链实际生成的 app/page bundles，验证原生 Feature 懒加载、manifest 名称匹配与跨 bundle 的一次性 Session 清理；这不替代固件渲染验证。

示例的 `pnpm test` 验证应用控制器与 echo 协议；包目录的 `pnpm test` 验证真实原始 Feature 边界（数值 handle、请求操作码、token、事件、负错误码），覆盖二进制校验、边界错误、同批事件、晚到事件、订阅清理及有界历史。`packages/astrolink` 下的 `pnpm test:install` 还会将真实 `.tgz` 用 npm 安装到仓库外的临时项目，检查 CJS、ESM 和 TypeScript 消费方式。fixture 不进入 RPK，也不证明固件 ABI、真实 BLE 链路或射频能力。实际设备验收应记录两台设备型号、固件、模块版本和操作日志。
