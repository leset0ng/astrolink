# Astrolink BLE 模块调查与设计草案

本报告只定义调查结论、架构和实现步骤，不包含模块实现。固件能力必须区分“协议允许”“静态发现”“ABI 已确认”和“两台设备实测通过”；下文设计参数均不代表已经获得设备能力。

**目标与范围**

为快应用的 QJS 提供扫描、广播、广播数据回调、连接、双向消息传输和接收回调。模块不注册页面、Launcher 或设置界面。开发者面对 Peer、Session 和双向 Link，不需要选择 GAP central/peripheral 或 L2CAP client/server。架构从第一版支持多 Link；首版验收和配置容量为一条活动连接，不能据此采用单例连接状态或单份收发缓冲。

**当前工程与 SDK 的已确认边界**

- 当前工程是 `no_std` Rust 模板，模块 ID 为 `astrolink`，`Canopus.toml` 是 `portable` / `removable` / ABI 2；`src/lib.rs` 尚无业务逻辑。
- 调查的本地 Canopus SDK revision 为 `844a1e03d6ab976448b71ea5bf4db0bd7d48630e`。
- `sdk/c/canopus_services.h:269` 和 `:472` 表明 `ctx->bt` 提供 MiWear 手机连接状态、WearPacket L2 发送和接收注册。这不是通用 GAP、GATT 或 LE L2CAP CoC API，不能直接用它实现两台手环互连。
- `ctx->bus` 同步调用订阅者，不提供跨任务队列；`ctx->ui->post` 面向前台 UI 心跳，不能直接据此保证 QJS、熄屏或后台调度。
- SDK 的 public headers/runtime/examples 检索未发现现成的 QJS native module 服务。需要独立确认固件的模块解析、原生函数绑定、JS owner task 投递和 context 销毁机制。
- 第一版预计需要 exact-target 后端。只有把完整 BLE/QJS 能力沉到稳定 Supervisor 服务以后，业务模块才具备保持 portable 的条件。
- 当前 `removable` 配置不能直接沿用为最终承诺。若 native module、JS 函数对象或固件回调仍引用模块代码，必须保持驻留；只有证明所有引用和在途任务均已清空才能热卸载。

**参考模块的精确调查结果**

参考仓库固定在 commit `e396a93f038d5c2414817a3ea5a9d7a35a847e4d`（2026-10-24）。它没有 QJS integration：native_app 是固件原生应用，外部音频接口是 `/dev/canopus_audio` VFS driver，不是 JS native module。因此只能参考蓝牙私有 ABI、资源所有权与生命周期，不能声称已经存在可复用的快应用导入通道。

必须避开的资源身份包括：`bluetooth_audio` 模块 ID、`/dev/canopus_audio`、原生 app `0xCB` / package `com.canopus.headphones`、`BAU1` query magic `0x42415531`、`BAUA` runtime magic `0x42415541`。Astrolink 不注册原生 app，无需申请另一套 UI 标识。标准 `canopus_mod_*` / descriptor 导出属于模块 ABI，各 ELF 可按 SDK 正常使用，不能为“避冲突”随意改名。

Audio 的 H4 compatibility hook 对完整 ACL 中的经典信令 CID `0x0001`、Configuration Request `0x04` 做处理：删除 `7F 01 01` 选项，并修正各层长度。匹配任何 dynamic destination CID，没有按 Audio 自身 ACL/CID 过滤。处理后直接转交 stock receive，不是任意第三方 hook 链；adapter ON 和部分连接操作会再次安装该 hook。

LE signaling CID `0x0005` 不满足这个解析器的匹配条件，因此从该 transform 的代码路径看会原样转发；这只证明包过滤条件，不证明两模块已经实机共存。Astrolink 必须优先使用 host API；若需要相同 H4 单槽入口，必须先解决统一所有权与分发，不能只各自保存旧指针后轮流覆盖。

另一个共享入口是系统 companion 配对回调。Audio 安装 `bt_install_pair_request_filter()`，在耳机地址与 pending 配对匹配时拦截 stock 回调，其余请求转发。该过滤函数只接收地址，没有 transport 参数；后续部分 bond 回调才明确区分 Classic。因此 Astrolink 不应复制或替换这张表，并需实测 Classic 耳机配对与 LE peer 配对重叠的情况。

Audio 在发布 adapter callbacks 后立即采用 resident 生命周期，deactivate/stop 返回 reboot-required，没有提供可直接复用的完整卸载清理范例。Astrolink 的 Session 可以独立关闭，但模块代码能否卸载仍要取决于回调撤销和异步任务排空证据。

参考项目的私有后端依赖 sibling `../Canopus/sdk/rust/canopus-target-private`，当前工程依赖 `../Canopus-Private`。公开参考仓库不包含该 facade 的底层实现，必须以本地实际 SDK 和 exact target records 为准。

精确源码：

- [Module descriptor](https://github.com/Searchstars/Canopus-Module-BluetoothAudio/blob/e396a93f038d5c2414817a3ea5a9d7a35a847e4d/crates/bluetooth-audio-device/src/module.rs#L136-L176)
- [VFS driver](https://github.com/Searchstars/Canopus-Module-BluetoothAudio/blob/e396a93f038d5c2414817a3ea5a9d7a35a847e4d/crates/bluetooth-audio-device/src/target/audio_device.rs#L177-L240)
- [H4 wrapper](https://github.com/Searchstars/Canopus-Module-BluetoothAudio/blob/e396a93f038d5c2414817a3ea5a9d7a35a847e4d/crates/bluetooth-audio-device/src/target/compatibility.rs#L10-L42)
- [Classic packet transform](https://github.com/Searchstars/Canopus-Module-BluetoothAudio/blob/e396a93f038d5c2414817a3ea5a9d7a35a847e4d/crates/bluetooth-audio-core/src/compatibility.rs#L15-L102)
- [Hook reinstallation](https://github.com/Searchstars/Canopus-Module-BluetoothAudio/blob/e396a93f038d5c2414817a3ea5a9d7a35a847e4d/crates/bluetooth-audio-device/src/target/bluetooth.rs#L779-L793)
- [Companion pairing filter](https://github.com/Searchstars/Canopus-Module-BluetoothAudio/blob/e396a93f038d5c2414817a3ea5a9d7a35a847e4d/crates/bluetooth-audio-device/src/target/bluetooth.rs#L41-L73)
- [Resident lifecycle](https://github.com/Searchstars/Canopus-Module-BluetoothAudio/blob/e396a93f038d5c2414817a3ea5a9d7a35a847e4d/crates/bluetooth-audio-device/src/module.rs#L94-L105)
- [External private backend dependency](https://github.com/Searchstars/Canopus-Module-BluetoothAudio/blob/e396a93f038d5c2414817a3ea5a9d7a35a847e4d/crates/bluetooth-audio-device/Cargo.toml#L37-L44)

**固件调查结果**

已在 Q66/Band 11 `4.100.155` 和 P67 `3.101.043` 的 Bluelet 核心确认真实 LE L2CAP CoC：有对应信令分派、MTU/MPS/credit 字段和每通道 SDU 重组。结论为“底层协议实现已证实，模块接入需补齐 ABI 与设备验证”；不是“两台手环已经可以互传”。P65 仅检查身份和字符串，`.177/.179` 未作本轮 BLE ABI 审计，不能沿用 `.155` 地址。

主要数据库为 `/Users/lesetong/develop/mifw/analysis-q66-177/155/q66-ap.i64` 和 `/Users/lesetong/develop/mifw/analysis-p67-043/vela_ap.bin.i64`。下面的地址均为静态分析入口/数据地址，不表示已获准生成 callable binding；函数入口实际调用还需核对 Thumb 与完整 ABI。

| 已恢复资源事实 | Q66 .155 地址 / 对照 | 设计含义 |
| --- | --- | --- |
| 全局 L2CAP owner，含 channel/link/PSM 注册链表 | `0x200BE890`；P67 `0x2013BDBC` | 一个 host 管理多链路和多通道，不能每条 Link 建一个栈 |
| 动态 LE PSM 注册/分配，范围 `0x80..0xFF` | `0x0C86A268`；P67 `0x0C800624` | 每服务注册共享监听 PSM，接收多条 Link，不按 Link 消耗 PSM |
| 栈内 CID 分配与全局去重，LE 分支 `0x40..0x7F` | 分配 `0x0C86D8FC`，查重 `0x0C86D8D0` | CID 是共享资源；64 个 CID 值不等于能建立 64 条 LE 连接 |
| 发送按 CID 取 channel，channel 自带 queue/credit/callback | `0x0C86E7F8`；channel+4 queue/credit state，+20 callback，+116/+118 本地/远端 CID | 后端和上层都应保留 per-channel 状态 |
| 每通道接收、credit 消耗/补充和 SDU 重组 | `0x0C873794`；P67 `0x0C803B88` | 已有底层流控实现；不能替代 JS 收件队列背压 |
| 广播创建与扫描创建都有独立 handle，存在多个系统业务调用点 | 广播 `0x0C46B5A0`，扫描 `0x0C46B68E` | 可以按资源句柄管理；并发数量和组合尚未证明 |

未恢复/未验证的硬件上限包括：controller 最大 ACL/LE 连接数、central/peripheral 并发组合、广告集数、扫描客户端数、全局 CoC buffer pool。`0x0C7ED414` 的数字 8 是连接列表上报截断阈值；`0x0C8715FC` 的 1..5 是单次 enhanced credit-based 请求的 channel group 数，两者都不能拿来当 LE Link 上限。

因此推荐首个验证 target 使用证据最完整的 Q66 `.155`，随后适配 P67 `.043`；实际设备固件若不同，应先补 exact target 调查。需要真实设备确认扫描与广播并行、已有手机 ACL 时增加 peer、Audio 播放并存、熄屏调度、完整 buffer/callback 所有权。

固件身份：Q66 AP SHA-256 为 `ea0bdf1920cb30223d616432af00565ca67622e6468328f5eab155f8cdc2fb9f`，build 为 `user-4.100.155-cn-202609041500`；P67 AP SHA-256 为 `519307675665e4866d722a8119a98589c397b614ac3294cb87bfc86de45756ec`，build 为 `CONBINE_LTALM078_T3.101.043`。这是解包 AP 的哈希，不是外层 full firmware 包哈希。

CoC 的证明不依赖既有函数名称：

| Q66 .155 VA | 反编译确认的行为 |
| --- | --- |
| `0x0C8732AE` | LE 分支构造 code `0x14`、CID `0x0005`、10-byte payload，写 LE_PSM/SCID/MTU/MPS/initial credits；classic 分支是 code `0x02`、CID `1` |
| `0x0C872F2C` | 解析 LE 请求，查 LE PSM registry，验证 SCID `0x40..0x7F`，读取 MTU/MPS/credits |
| `0x0C872CC2` / `0x0C87306A` | 构造/解析 code `0x15` 的 LE CoC 响应，设置 CID、MTU/MPS/credits 和连接完成状态 |
| `0x0C872EDA` | Flow Control Credit 更新，处理 credit 溢出并唤醒待发队列，产生 event `17` |
| `0x0C873794` | signaling CID `5` 分派与 SDU 重组，完整 SDU 交付 event `7`；接收 credit 降到 `<=10` 时自动发送 code `0x16` 补 5 credits |

候选可接入边界：PSM register `0x0C86EF90`、unregister `0x0C86EFCC`、connect queue `0x0C86F010`、入站 accept/reject queue `0x0C86F038`、disconnect `0x0C86F094`、send buffer queue `0x0C86F0BC`。这些已经看到函数体，但不代表完整 ABI 已恢复。约 68-byte connect request 观察到 local CID+0、PSM+2、transport flag+8、callback+12、address+16、local MTU+52、initial credits+60、local MPS+62；其余字段、flag 枚举、失败与回调所有权仍需审计，禁止直接套 classic 请求布局实现 FFI。

观察到 callback event `1/2/3/7/9/17`，分别关联入站、出站请求处理、通道就绪、完整 SDU、动态 PSM 注册完成和 credits 更新；event union、断开/失败事件和释放责任未完整恢复。固件自动补 credits，因此不能靠延迟 JS 消费来阻止底层继续接收。模块需要有界接收队列及明确的溢出处理；这项证据本身不要求新增应用消息窗口协议。

GAP 静态入口：

| 能力 | Q66 .155 证据 / 限制 |
| --- | --- |
| 扫描 | create `0x0C46B68E`，候选 `(instance, scan_params, optional_filter, callbacks) -> handle`，IPC 237；stop `0x0C46B730` / `0x0C46B710`，IPC 238；SAL `0x0C47140C` |
| 广播 | create `0x0C46B5A0`，候选 7 参数，params 28 bytes，包含 adv/scan response 及 callback，IPC 67；stop `0x0C46B664` / `0x0C46B644`，IPC 68 |
| 原始扫描结果 | 系统 callback `0x0C7EE09C` 有地址、RSSI、packet type、AD；模块应注册自己的扫描 callback，避免 MiWear 自有解析过滤 |
| LE connect | server `0x0C46D20C` 的 opcode 106 分支 `bt_sal_le_connect` → `0x0C87B374`；完整客户端 wrapper ABI 尚未定位 |
| LE disconnect | 同 server opcode 107 `bt_sal_le_disconnect` → `0x0C870AE8` → `0x0C87ED6C`；完整客户端 wrapper ABI 尚未定位 |
| 执行任务 | `0x0C473178` 创建 `bluelet_thread`，优先级 103、栈配置 `0x2000`，初始化 `/dev/ttyBT0` 和协议 owner |

广告 IPC 对每块数据接受 `<=256` bytes 只说明封装上限，不证明 controller 可发 256-byte 广告；系统支付广播 caller 明确使用 31-byte legacy payload。

P67 对照：PSM register/unregister `0x0C805A80` / `0x0C805AC4`；connect queue `0x0C805B0C`；send queue `0x0C805BE8`；LE code `0x14` handler `0x0C802AEC`；code `0x16` handler `0x0C803038`；接收/重组 `0x0C803B88`。存在同类实现，但必须分别适配。

**QJS 原生扩展路径的静态发现**

固件已有 Feature 框架。首选调查方向是接入该 registry，并沿用固件 Feature 的 require/销毁规则，而不是假设可直接挂 upstream `JS_NewCModule` 或任意 import resolver。

| Q66 .155 VA | 已确认语义 |
| --- | --- |
| `0x0C495F2C` | `FeatureRegisterFeature(registry, description)`，按 description+4 的名字注册，保留 description 指针，调用注册 hook |
| `0x0C49A7F0` | 按名字查 Feature description |
| `0x0C4A0964` | `FeatureManagerQjs::findFeature` |
| `0x0C4A0A54` | `FeatureManagerQjs::featureRequire`，创建实例并调用 onRequired |
| `0x0C4A0B7C` | `FeatureManagerQjs::uninit`，清理实例和原型，调用 onDestroy |
| `0x0C74957C` | Application callable 投递包装候选，调用 application virtual slot+80 |
| `0x0C7419B0` → `0x0C73700C` | 带 mutex 的 task vector 入队；空转非空时唤醒 |
| `0x0C3F5700` | `uv_async_send`，有 libuv async 源码断言和唤醒 FD 写入 |
| `0x0C741080` | 任务执行前检查 runtime 停止标志，停止时 drop task |

P67 `FeatureRegisterFeature` 对照为 `0x0C862898`。这些证据支持“有注册、投递和销毁接入路径”，但 FeatureDescription/member ABI、JSValue 布局、registry 获取、manifest 权限、动态注销和 module unload 安全性均未完整恢复。

`0x0C721680` 虽涉及 `@aiot/`/system module resolver，但属于 compiler 路径，不能据此断言运行时接受 `@lesetong/astrolink`。最终 import/require 拼写、快应用构建期可见性需在 P0 验证；本文 API 名称表达开发者契约。

P0 建议先以双机 native SDU echo 验证射频链路，同时用独立 QJS 探针验证 Feature 注册、任务投递、Promise 和销毁；两项通过再整合，不把复杂业务混入 ABI 调试。

**模块架构**

```text
QuickApp / QJS owner task
    Session(serviceId)
        +-- Link A: state / pending sends / received messages
        +-- Link B: state / pending sends / received messages
             |
       Exact-target backend
       GAP / ACL / LE CoC / task dispatch
             |
       Existing firmware Bluetooth host
```

代码按 `qjs`、`core`、`backend` 分工即可，不预先拆出独立协议框架或多个 crate。`qjs` 负责原生绑定、Promise 和回调；`core` 负责 Session、连接表和有界消息队列；`backend` 隔离各固件 ABI、buffer 所有权和 Bluetooth 任务调用。

Bluetooth 回调将收到的数据移交到有界队列，QJS 对象创建、回调和 Promise 完成都在所属 QJS 任务执行。每个 Session 归创建它的 context 所有；context 销毁时关闭它的资源，用简单的 generation 校验丢弃晚到事件。这里需要生命周期管理，不增加应用身份认证或新的权限系统。

多连接架构从首版保留以下内容：

- 一个连接表，每条 Link 独立维护状态、发送顺序、待发送/接收队列和超时。底层 CID、MTU/MPS 和 credit 由该 Link 的后端通道管理，不在 core 再维护一套 credit。
- Link 使用模块分配的 ID，通过后端通道映射路由事件；连接关闭后使旧映射失效。不预先设计多 controller 的通用复合键。
- 设备连接表容量为 16，由最多 4 个 Session 共享。每 Link 仍有 4 条待发送消息和 4 条接收消息的队列上限；全部 Link 的 TX/RX 共用固定 40 个消息槽。全局预算耗尽时，发送返回 `BUSY`，接收以 `RECEIVE_OVERFLOW` 关闭该 Link，不截断或静默丢弃消息。
- 有待发送数据的 Link 简单轮询，每次提交一条消息；某条 Link 暂不可发送就处理下一条，不引入带权或 deficit 调度器。
- Session 保存自己申请的扫描/广播 handle 和服务监听注册。首先使用固件已有的多客户端机制；资源不足返回 BUSY，不先实现扫描参数合并、广告轮换或通用租约管理器。
- 关闭 Link 首先关闭自己的 CoC。ACL 由固件 host 管理；只有确认 ACL 是模块独占且已无本模块通道使用时才主动断开，不干预手机或 Audio 连接。不预先增加跨模块 ACL 引用计数框架。
- 一个服务监听注册可以接受多条连接；每条 Link 首版使用一个 CoC。入站连接由已注册 SPSM 路由给对应 Session；相同 serviceId 的重复 Session 注册在首版返回 BUSY，不增加共享服务模式。

用 host 后端构造至少两条 Link，验证消息不会串线、一条断开不影响另一条，以及一条暂停发送时另一条仍可推进。Host 测试同时覆盖 16 个表项全部分配、关闭和复用，以及消息池耗尽与关闭后回收。

当前 `bluelet.rs` 的 `OwnerState.connecting` 每个 Session 仅允许一个在途主动连接，因此同一 Session 的多条 Link 仍须串行建立。16 条 Link 是静态表容量；本次调整未构建设备包或执行硬件验证，仍需 Mirage 18/18 生命周期探针和双设备 peer 运行验证。

**连接抽象与状态机**

```text
connecting -> opening CoC -> checking service -> connected
                  error / disconnect -> closed
```

扫描和广播独立于 Link 状态。主动 `connect(peerId)` 返回 Link；接收端通过 `onConnection` 得到同一种 Link。开发者不选择 GAP 或 CoC 角色，两端使用相同的 `send/onData/disconnect`。

`startAdvertising()` 内部注册服务监听；停止广播后停止接收新连接，已有 Link 继续使用。`connect()` 内部完成 ACL、CoC 和服务确认。服务不匹配或资源不足时返回明确错误。

同一 Session 对同一 peer 重复 connect，复用已有 Link 或在途连接操作。首版不保证两端同时 connect 自动合并：冲突时允许返回 BUSY，由调用者稍后重试；先验证固件实际行为，再决定是否需要自动收敛。去掉为此预设的随机 nonce、双候选预算和多轮退避协议。

peerId 是扫描结果提供的不透明标识，模块保存地址与地址类型。第一版不增加专门的过期定时器和 PEER_EXPIRED 错误；地址变化导致连接失败时重新扫描。

**广播与服务发现协议建议**

1. `createSession` 必须显式填写非空 `serviceId: string`，例如 `chat` 或随机字符串 `k7m2p9`。不要求 UUID、包名前缀或集中注册，也不自动生成。
2. 两端按 UTF-8 字节精确匹配 serviceId。随机字符串生成一次后双方共用；同一 serviceId 可以对应多个 peer 和 Link。
3. 广播携带协议版本、serviceId、已注册的动态 SPSM 和用户数据。地址与地址类型已由扫描报告提供，首版不额外增加随机广播 token。
4. 动态 LE SPSM 由后端注册，成功后才广播。合法 AD 类型与具体字节布局在 P0 核对后冻结，普通字符串不能直接填入规范要求 UUID 的 Service Data 字段；不为 serviceId 增加 UUID 映射或哈希机制。
5. 首版只暴露一份 `data`，并保证每次回调交付完整数据。先按单个 legacy 广播 payload 的空间预算设计，不增加跨广播/scan response 的自定义分片、配对和 complete 状态；如需使用 scan response，后端必须在其布局内完成完整报告，不能让应用收到半份数据。
6. serviceId 与 data 均受实际广播空间约束。公布字节上限，超限报错，不截断。广播里不必再放一份可单独查询的实例身份。
7. 同一 peer 的相同广播数据默认去重，数据变化再回调；提供 `allowDuplicates` 供需要持续 RSSI/广播报告的应用使用。不预设复杂的时间合并策略。

serviceId 只用于业务匹配。

**连接内协议建议**

使用 LE legacy Credit Based Flow Control Mode。它已提供双向、有序的 SDU 交付和基于 credit 的底层分段/流控，首版尽量沿用这些能力。

通道建立后双方各发送一条小型 HELLO，内容只有协议版本与 serviceId。双方确认一致后向应用交付 Link，不做版本范围、可选特性、nonce、消息窗口或密码协商。HELLO 的实际编码需落在所用通道的 MTU 内；serviceId 长度限制同时满足广播和握手要求。

握手后，**一次 send 对应一个 CoC SDU，一次 onData 对应一条完整 SDU**：

- 不再给每条数据加 16-byte 头，也不设计 DATA/ACK/CLOSE/ERROR 信令、messageId 或 offset/totalLength。
- L2CAP 完成 SDU 内的 MPS 分段与重组。首版不接受超过对端 receive MTU 或模块消息上限的数据，超限返回 INVALID_ARGUMENT，不增加跨 SDU 大消息传输。
- 每条 Link 暴露 `maxMessageBytes` 作为该方向的发送上限；两端限制可以不同。后端在建通道时将本端 receive MTU 设为实际可接收的大小，连接接受策略不得把对端 receive MTU 当成本端接收上限。
- 数据接口使用 ArrayBuffer 或 Uint8Array，收到数据统一为 ArrayBuffer；文本和 JSON 由快应用自行编码。不增加文本/二进制类型协商。

`send()` 在固件栈成功接管该消息、模块不再持有提交责任后完成；如果后端有必要等待可提交事件，就在内部等待。它只表示本地提交成功，不承诺对端 JS 已收到或业务已处理。业务需要确认时自己回包。首版不增加模块级 ACK、发送回执、跨连接重试或 DELIVERY_UNKNOWN 协议。

固件会自动补 credits，因此接收队列仍需有上限。正常情况下投递到 QJS；队列溢出时关闭该 Link，并报告 RECEIVE_OVERFLOW，不静默丢包、不影响其他 Link。这个明确的失败语义足够用于首版；只有实测说明正常业务需要持续背压而不能接受断开时，再考虑额外消息窗口。

发送队列同样有上限，满时返回 BUSY；应用可以串行 await send，或自行处理 BUSY，不增加 onDrain。每 Link 保序和简单轮询足够，不承诺跨 Link 的全局消息顺序。队列与 buffer 的实际数量/大小在 P0 探针后确定。

模块首版不新增认证、密钥或加密握手。是否存在固件链路加密作为验证记录，不把未恢复的安全能力做成可用 API 承诺。

**QJS API 草案**

`@lesetong/astrolink` 是建议名称，最终导入方式需在 P0 验证。Session 的 serviceId 必填；Session 自己拥有一组扫描/广播状态，因此无需再返回两种操作句柄对象。

```ts
interface Astrolink {
  getCapabilities(): Capabilities;
  createSession(options: { serviceId: string }): Session;
}

interface Session {
  onAdvertisement(cb: (event: AdvertisementEvent) => void): Unsubscribe;
  startScan(options?: { allowDuplicates?: boolean }): Promise<void>;
  stopScan(): Promise<void>;
  startAdvertising(options?: { data?: ArrayBuffer | Uint8Array }): Promise<void>;
  stopAdvertising(): Promise<void>;
  onConnection(cb: (link: Link) => void): Unsubscribe;
  connect(peerId: string): Promise<Link>;
  close(): Promise<void>;
}

type Unsubscribe = () => void;

interface Link {
  readonly id: string;
  readonly peerId: string;
  readonly maxMessageBytes: number;
  onData(cb: (data: ArrayBuffer) => void): Unsubscribe;
  onDisconnect(cb: (reason: string) => void): Unsubscribe;
  send(data: ArrayBuffer | Uint8Array): Promise<void>;
  disconnect(): Promise<void>;
}

interface AdvertisementEvent {
  peerId: string;
  serviceId: string;
  data: ArrayBuffer;
  rssi: number;
}

interface Capabilities {
  supported: boolean;
  maxLinks: number;
  maxServiceIdBytes: number;
  maxAdvertisingDataBytes: number;
}
```

`maxLinks` 是当前配置容量，首版为 1；固件实际上限留在探针记录中，不同时暴露另一套 backendMaxLinks。`maxServiceIdBytes` 是可接受的 serviceId UTF-8 上限；`maxAdvertisingDataBytes` 取支持长度内所有 serviceId 都可使用的保守固定 data 上限，避免要求开发者计算 AD 布局，具体值在布局确定后填入。supported 只回答当前设备是否支持完整基础功能，不增加逐项底层能力矩阵。

| 用户要求 | 对应 API | 约定 |
| --- | --- | --- |
| 扫描 | startScan / stopScan | 过滤当前 serviceId；只停止自己的扫描 |
| 广播 | startAdvertising / stopAdvertising | 内部维护监听；只停止自己的广播与新连接接收 |
| 注册广播数据回调 | onAdvertisement | 交付一份完整用户 data，返回取消订阅函数 |
| 连接 | connect / onConnection | 主动通过 Promise、被动通过回调获得同一种 Link |
| 传输 | link.send | 一次调用发送一条二进制消息，本地提交后完成 |
| 注册传输数据回调 | link.onData | 按完整 SDU 交付 ArrayBuffer |

错误先保留实际需要的 UNSUPPORTED、INVALID_ARGUMENT、BUSY、TIMEOUT、DISCONNECTED、RECEIVE_OVERFLOW。底层连接失败保留原始错误码供日志定位，不为尚未实现的认证、交付和 peer 过期机制建立错误分类。

连接使用一个内部默认超时，先不为扫描、广播、每次 send 都增加 timeoutMs 配置。scan/advertise 由显式 stop 或 Session.close 结束。stop、disconnect、close 和取消订阅可以重复调用；重复 start 返回 BUSY。

`onConnection` 只通知入站连接，避免主动连接同时通过 Promise 和事件通知两次。回调先注册再启动广播。Link 交付后至 JS 注册 onData 之间暂存有限消息，超限按 RECEIVE_OVERFLOW 处理；不为此增加窗口协议。

应用退出时关闭其 Session，停止它的扫描/广播并关闭 Link，结束未完成的操作。清理 JS 引用发生在所属 QJS 任务；晚到事件不得再访问已销毁 context。模块代码首版按驻留方式规划，Session 关闭不等于热卸载模块；不把通用热卸载作为首版工作。

**审查后的范围收敛**

首版保留多 Link 表、独立队列、简单轮询、资源所有权、QJS 任务隔离和有限缓冲。先不做应用层 ACK/窗口、大消息跨 SDU 分片、随机 token 去重协议、通用无线资源 broker、复杂调度、认证机制、自动重连或热卸载。需要这些能力时以具体业务和设备结果为依据追加。

**与 BluetoothAudio 共存的设计要求**

- 使用模块 ID `astrolink`、独立 native import 名和 `astrolink_*` 内部业务导出；模块私有目录/KV、日志和状态记录都归自身所有。SDK 规定的模块 ABI 导出保留原名。
- 使用自己注册的 LE SPSM、广告实例、扫描订阅和 GAP 连接；不使用经典 A2DP/AVDTP/AVRCP/SDP 的服务标识或通道。
- 不关闭共享蓝牙 adapter，不清空系统配对，不停止其他使用者的广播/扫描，不断开手机或耳机 ACL。
- 优先固件正式 host API 和多订阅回调。如果固件入口是全局单槽，必须先证明可以复用已有公共分发；不能各自覆盖 H4 receiver、L2CAP 信令处理器或系统配对回调。
- 需要统一入口时只增加最小共享分发能力，并让原有 stock 路径和 BluetoothAudio 保持正确所有权；不要复制出第二套直接操作 controller 的 host。
- 实测记录两种模块加载顺序及工作重叠的结果。运行时错误反映实际的资源冲突或后端不支持；验证记录是否齐全属于研发结论。

**实现规划与验收门槛**

| 阶段 | 工作内容 | 完成依据 |
| --- | --- | --- |
| P0 能力探针 | 选一个 exact target；确认 BLE API/事件 ABI/任务/所有权；证明快应用 native import、Promise 完成和 context 销毁；两台设备打开真实 LE CoC | 无 UI 探针日志与无线/设备证据；明确 phone link 和 Audio 共存结果 |
| P1 最小双向链路 | 从第一版使用连接表、per-Link 队列与简单轮询；扫描、广播、监听、连接、服务确认、双向 SDU；设备容量先配置 1 | 双机单 Link 互发；host 双 Link 隔离通过；主动/被动 Link 收发 API 相同；连接冲突能结束并返回结果 |
| P2 QJS 完整契约 | 六项接口与取消订阅、二进制消息、长度限制、有限队列、溢出断连、应用退出 | 快应用端到端收发与清理通过；send 完成和失败语义明确，无新增 ACK/分片协议 |
| P3 稳定性与共存 | 断连、乱序事件、后台/熄屏、堆压力、Audio/手机并存、模块启停 | 双机长时传输、没有回调悬空或持续堆增长；原厂业务可正常使用 |
| P4 扩展适配 | 新固件逐一恢复 ABI；根据实测上调连接容量并完成多设备验收；按能力开放更大广告；必要时下沉稳定公共服务 | 每 target 独立证据；扩容无需重构 Link 模型；不从相同版本后缀推断地址/布局相同 |

Host 测试覆盖服务确认、消息长度边界、队列上限、双 Link 隔离、晚到事件和关闭竞态；不为已交给固件的 L2CAP 分段/重组再建一套协议测试。ABI 检查关注真实调用约定和 buffer 所有权。Host/emulator 结果不能代替射频实测。

设备矩阵分两批：首版验收覆盖双向消息、长度边界、应用退出、断连、接收队列溢出及手机/Audio 共存；后续稳定性再扩展到扫描广播重叠、两端同时 connect、设备重启、地址变化、远离后重连和熄屏。记录吞吐、延迟和堆使用；需要后台常驻或耗电目标时，再做相应专项测量。

如果 P0 发现现有 CoC 无法安全接入或无法满足设备连接组合，不能把“固件内部有实现”当成可交付后端。保留相同 Session/Link API，单独评估 GATT write/notify 承载是否值得采用；不自动退到自建 HCI host 或复制 Audio 的经典 L2CAP。是否切换承载应由 P0 的具体结果决定。

**主要资料**

- 当前工程：`Canopus.toml`、`Cargo.toml`、`src/lib.rs`、`README.md`。
- 本地 SDK：`sdk/c/canopus_services.h`、`sdk/c/canopus_abi.h`、`docs/external-module-migration.md`、`docs/target-private-codegen.md`、`docs/band11-bluetooth-139.md`。
- Bluetooth SIG Core 6.1 L2CAP：https://www.bluetooth.com/wp-content/uploads/Files/Specification/HTML/Core-61/out/en/host/logical-link-control-and-adaptation-protocol-specification.html
- QuickJS 官方文档：https://bellard.org/quickjs/quickjs.html （用于 threading/JSValue ownership 原则，不代表目标固件 ABI 与上游一致）。
- 参考项目：https://github.com/Searchstars/Canopus-Module-BluetoothAudio ，固定 commit `e396a93f038d5c2414817a3ea5a9d7a35a847e4d`，关键源码链接见上文。
