# Web Admin 管理端 — 统一双入口实施计划

> 版本: 3.1 | 更新日期: 2026-05-03
> 关键结论：Web Admin 不是第二套业务系统，而是 API Switch 的第二个入口。后续开发必须围绕 **后端 Service 复用 + 前端 Feature UI 复用 + Adapter 双实现 + 单一 Binary 运行策略** 展开，避免桌面端和 Web 端长期维护两套业务逻辑。

---

## 1. 项目目标（重排）

API Switch 本次开发目标是：

- 支持 **桌面管理**：Tauri GUI
- 支持 **Web 管理**：浏览器 + HTTP Admin API
- 支持 **有桌面系统 / 无桌面系统** 两种运行环境
- 保证 **一套业务逻辑 + 一套业务 UI**，避免桌面/Web 两套开发

核心约束：

```text
两种管理方式，一个系统内核。
禁止两套业务逻辑，禁止两套业务 UI。
```

### 1.1 运行方式底线

本项目的明确底线是：

```text
优先保证 Desktop + Web 的统一工作方式。
CLI 不是当前主线目标。
如未来需要无头运行，也必须优先尝试在同一个 binary 内完成。
如果做不到单一 binary，则当前阶段不单独建设 CLI 产品形态。
```

也就是说：

- 当前必须完成的是：**一个 binary，同时支持桌面环境与无桌面环境可用**
- 在有 GUI 环境下：运行 **Combined** 模式（桌面窗口 + Web Admin）
- 在无 GUI 环境下：运行 **Standalone** 模式（仅 Web Admin / 代理服务）
- 该 Standalone 仍优先视为**同一程序的运行模式**，不是独立 CLI 产品线

### 1.2 最终形态

```text
后端：
DAO
 ↓
Service 层                    ← 唯一业务逻辑
 ↓                       ↓
Tauri Commands          Admin HTTP Handlers
 ↓                       ↓
桌面端 Adapter           Web Admin Adapter

前端：
ApiAdapter interface
 ↓                       ↓
tauriApiAdapter          webAdminApiAdapter
 ↓                       ↓
useApiAdapter()
 ↓
features/*               ← 共享业务 UI
 ↓                       ↓
Desktop Pages            Web Admin Routes
```

### 1.3 运行模式

```text
detect_runtime_mode()
  ├─ Combined   -> GUI + Web Admin
  └─ Standalone -> Web Admin only
```

运行模式优先级固定为：

1. CLI 参数（如未来支持 `--headless` / `--combined`）
2. 环境变量（如 `API_SWITCH_HEADLESS=1`）
3. 自动环境检测结果

自动检测只是默认策略；显式覆盖始终优先。

---

## 2. 不偏离原则（升级版）

后续所有 Web Admin / 双入口开发必须遵守以下原则。

### 2.1 后端原则

禁止：

```text
commands/channel.rs 写一套业务逻辑
admin/channel_handlers.rs 再写一套业务逻辑
```

必须：

```text
commands/channel.rs -> 调 services/channel_service.rs
admin/channel_handlers.rs -> 调 services/channel_service.rs
```

也就是说：

```text
Service 是唯一业务逻辑来源。
```

### 2.2 前端原则

禁止：

```text
src/pages/ChannelPage.tsx       桌面一套
src/web-admin/ChannelPage.tsx   Web 再写一套
```

必须：

```text
src/features/channels/ChannelManager.tsx 共享一套
src/pages/ChannelPage.tsx 只是桌面壳
src/web-admin/src/WebAdminApp.tsx 只是 Web 壳/路由
```

新增强约束：静态 UI 一致性

```text
对于共享业务界面中的标题、文案、布局、样式、结构性组件调整，
必须通过共享 Feature / Shared Shell / Shared Tokens 完成一次修改。

验收时：
- Desktop 构建后应体现该变更
- Web 构建并刷新后也应体现该变更
- 若 Web 端需要再做一次等价修改，视为双端 UI 已分叉，不符合“第二入口”目标
```

边界说明：

- 允许不同：登录页、Web 路由壳、浏览器鉴权壳、桌面窗口/tray/system 集成
- 不允许不同：共享业务页面中的静态标题、按钮文案、列表布局、表单布局、共享错误/空状态、共享样式 token

### 2.3 API 调用原则

禁止页面直接依赖具体运行环境：

```ts
invoke("list_channels")
fetch("/admin/channels")
```

必须通过 Adapter：

```ts
const api = useApiAdapter();
api.channels.list();
```

桌面和 Web 分别实现同一个接口：

```text
src/lib/tauriApiAdapter.ts
src/lib/webAdminApiAdapter.ts
```

### 2.4 启动模式原则

- 启动期必须完成一次统一的 runtime mode 判定
- Combined 模式允许桌面副作用（窗口、tray、前端事件）
- Standalone 模式不得依赖窗口、tray、桌面生命周期
- 任何功能如果只在 Combined 可用，必须明确标注为桌面副作用，而不是核心业务能力

### 2.6 新增功能开发规则（补充）

对于本计划后续新增的功能，除了遵守 Service / Feature / Adapter / 单一 Binary 原则外，还必须遵守以下开发规则：

1. **先定义数据流，再写 UI**
   - 明确触发入口（Desktop / Web / 后端定时 / 外部请求）
   - 明确传输路径（Tauri command / HTTP Admin API / Adapter）
   - 明确展示入口（Desktop / Web 哪一侧负责展示）

2. **新增功能优先走共享 feature，不得先写双端页面**
   - 可以先有 Desktop 壳层入口和 Web 壳层入口
   - 但业务主视图、表单、列表、静态文案、布局必须优先放在共享层

3. **若当前阶段只能做“半闭环接线”，必须明确标记状态**
   - 可分为：`前端骨架已接` / `后端未完成` / `联调未完成` / `构建未验证`
   - 禁止把“骨架已接”写成“功能已完成”

4. **任何新增跨入口功能都必须写入计划中的“差距 / 分期 / DoD / 验收清单”**
   - 不能只在代码中推进，不回写计划

---

## 3. 当前状态与偏移检查

### 3.1 已对齐部分

以下内容已作为 Web Admin 基础设施保留：

- `web_admin_enabled`
- `web_admin_username`
- `web_admin_password`
- `web_admin_port`
- AdminServer
- 单端口 / 双端口模式
- `/admin` 静态入口
- `/admin/login`
- `/admin/logout`
- `/admin/settings`
- `/admin/status`
- `/admin/audit-logs`
- Bearer Token 鉴权
- 登录失败锁定
- 审计日志基础
- CORS 本地白名单
- Web Admin 构建脚本
- 静态资源编译期嵌入

当前已完成一次前端复用试点：

```text
src/features/settings/SettingsEditor.tsx
```

桌面端和 Web Admin 均使用同一个 `SettingsEditor`。

以下结构也已经朝目标靠拢：

- `src/features/channels/ChannelManager.tsx` 已作为共享业务 UI 使用
- `src/pages/ChannelPage.tsx` 已收敛为桌面壳
- `src/lib/useApiAdapter.ts` 已具备运行环境判断与 adapter 切换
- `src-tauri/src/services/channel_service.rs` 已被 command 与 admin handler 复用

### 3.2 当前偏移点

虽然基础方向正确，但当前仍存在以下偏移：

1. **自动环境检测尚未成为主启动逻辑**
   - 当前仍以 `web_admin_enabled + 用户名密码` 作为主要开关
   - 还未上升为“系统环境自动决策 Combined / Standalone”的统一编排

2. **无头环境可用性尚未闭环**
   - 代码中已有 `Option<tauri::AppHandle>` 等准备
   - 但整体入口仍是 Tauri 窗口流程，headless 场景的默认运行策略未完全收束

3. **计划文档主线仍不够聚焦**
   - 需要明确：Desktop + Web 是当前主目标
   - CLI 不是本轮主交付；若未来需要，也只能作为同一 binary 的附加运行模式后置考虑

### 3.4 新增功能开发状态记录：Translation Relay / 翻译转发（新增）

本节用于记录“桌面侧触发自动翻译 -> 转发到 Web Admin 端口 -> Web UI 显示结果”这一新增能力的当前真实开发状态。

#### 目标定义

目标不是单独做一个 Web 页，而是补一条新的双入口数据流：

```text
Desktop 左侧入口
  -> 触发自动翻译
  -> 结果转发到 Web Admin 端口
  -> Web UI 读取并显示最近一次结果
```

该功能的定位：

- 它是**新增功能开发规则**下的示例功能
- Desktop 负责触发
- Web 负责读取与展示
- 仍应遵守 shared feature + adapter + 单一后端状态来源

#### 已补充到开发中的内容（当前仅属“骨架已接”，不可视为完成）

前端已开始接线：

1. 新增共享功能文件：
   - `src/features/translator/TranslationRelayPanel.tsx`
   - 内含：
     - `TranslationRelayPanel`（Desktop 发送面板）
     - `TranslationRelayView`（Web 展示视图）

2. Desktop 导航已开始接线：
   - `src/features/shell/MainShell.tsx` 已加入 `translator` 左侧导航项
   - `src/App.tsx` 已开始把 `translator` 路由接到桌面主区

3. Web 展示页已开始接线：
   - `src/web-admin/src/WebAdminApp.tsx` 已开始加入 `translator` 页分支
   - 已开始加入轮询读取最近一次翻译结果的结构

4. 共享类型与 adapter 已开始扩展：
   - `src/types.ts` 已开始增加 `TranslationRelayPayload` / `TranslationRelayRequest`
   - `src/lib/apiAdapter.ts` 已开始增加 `translation.getLatest()` / `translation.translateAndRelay()`
   - `src/lib/api.ts`、`src/lib/tauriApiAdapter.ts`、`src/lib/webAdminApiAdapter.ts`、`src/web-admin/src/api.ts` 已开始接相应调用骨架

5. i18n 文案已开始补充：
   - `en.json` / `zh.json` 已开始增加 `nav.translator` 与翻译页面基础文案

#### 当前明确未完成的部分

以下内容目前仍未闭环：

1. **Rust 后端状态未完成**
   - 尚未正式落地“最近一次翻译结果”的共享状态槽位
   - `AppState` / `AdminState` 还需要明确注入该状态

2. **真正的自动翻译逻辑未完成**
   - 当前只接了前端骨架
   - 后端尚未完成翻译服务策略
   - 仍需决定翻译实现：
     - 外部翻译 API
     - 现有模型/渠道翻译
     - 其他受控实现

3. **Tauri command 未完成**
   - 仍需补：
     - `translate_and_relay`
     - `get_translation_relay`

4. **Admin HTTP 端点未完成**
   - 仍需补：
     - `GET /admin/translation-relay`

5. **命令注册 / 路由注册 / 后端联调未完成**
   - `src-tauri/src/lib.rs`
   - `src-tauri/src/commands/mod.rs`
   - `src-tauri/src/admin/router.rs`
   - 仍需继续接线

6. **构建与类型检查未完成**
   - 当前还不能宣称该功能已可用
   - `pnpm typecheck` / `pnpm build:web-admin` / `pnpm build:renderer` / `cargo check` 仍需验证

#### 当前状态判定

统一标记为：

```text
Translation Relay：前端骨架已接；后端未完成；联调未完成；构建未验证。
```

这意味着：

- 当前可以把它纳入开发计划
- 但不能把它写成“已完成功能”
- 后续必须作为一个明确收口项继续推进

---

## 4. 最优先业务目标：API 渠道 Channels

第一核心业务模块是：

```text
API 渠道管理 Channels
```

因为没有 Channels，Web Admin 没有实际管理价值。

目标：

- 桌面端 Channel 页面继续可用
- Web Admin 可以管理同一批 Channels
- 两端使用同一套 `ChannelManager` UI
- Tauri command 和 HTTP API 使用同一套 `channel_service`

---

## 5. 开发次序重排（先稳桌面主路径，再扩 Web，再补模式治理）

这一节用于明确**真正执行时的开发先后顺序**。原因是：

- 当前代码里 Web Admin 基础设施、Channels 共享、Settings 共享已经有一定基础
- 如果仍按抽象的 A/B/C/D 顺序推进，容易忽视“桌面主路径优先”和“已有成果先收口”的现实
- 个人系统场景下，更合理的顺序是：

```text
先锁住桌面主体验与当前已成型的共享模块，
再把 Web 最小闭环补齐到稳定可用，
再补自动模式治理与无桌面闭环，
再扩剩余业务面，
最后做工程化与 CLI 后置评估。
```

### 5.1 执行总顺序

#### Step 1：锁桌面主路径，不允许回退

先做以下确认与收口：

1. Tray 行为保持稳定
2. Channel 页面继续可用
3. Settings 页面继续可用
4. Proxy 启停继续可用
5. 设置修改后的 tray / proxy / admin 联动行为不回退

这一步不是“新增功能”，而是把桌面主路径作为后续所有改动的回归基线。

#### Step 2：收口当前已存在的 Web 最小闭环

优先把已经有基础的内容补齐到“稳定可用”而不是继续散开扩展：

1. 登录 / 登出 / 鉴权链路
2. `/admin` 静态入口
3. Settings 读取 / 保存
4. Channels 列表 / 创建 / 编辑 / 删除
5. `fetch_models` / `select_models` / `probe_url`
6. Web 前端与 HTTP API 的错误反馈闭环

也就是说，先把当前已经做了 60%-90% 的模块收成一个真正可用的最小版本。

#### Step 3：补运行模式治理，但第一版以“能用”为目标

在桌面主路径稳定、Web 最小闭环稳定之后，再处理：

1. `RuntimeMode` 统一定义
2. `detect_runtime_mode()`
3. env / 参数覆盖优先级
4. 无 GUI 情况下进入 Standalone
5. 模式日志与可观测性

这里第一版不追求“所有平台下 100% 精准自动检测”，而追求：

- 有明确规则
- 有手动覆盖
- 无桌面环境可以稳定进入可用状态

#### Step 4：扩 API Pool / Tokens / Logs 等个人系统高频模块

在最小闭环成立后，再扩真正高频管理能力：

1. API Pool
2. Access Tokens
3. 基础 Logs
4. Web 代理状态与启停

这一步要沿用已验证的统一路径：

```text
service 抽离 -> HTTP API -> shared feature UI
```

#### Step 5：补体验、一致性、Tray 制度化保障

在核心功能面可用后，再集中做：

1. Tray 回归门禁制度化
2. Settings / Web 导航优化
3. 错误提示统一化
4. Dashboard / 状态视图增强
5. 双入口端到端一致性回归

#### Step 6：工程化封口

最后再做：

1. 防分叉规则
2. CI / review 检查
3. 自动化一致性回归
4. 模式可观测性补全

#### Step 7：CLI 后置评估

只在前面全部稳定后才进入。

---

### 5.2 为什么这样排序

#### 原因 1：当前已有成果应优先收口

当前代码里已经明确存在：

- `channel_service`
- `ChannelManager`
- `SettingsEditor`
- `webAdminApiAdapter`
- Admin 路由与鉴权骨架

因此最合理的顺序不是先大改架构，而是先把这些已有成果收敛成**可稳定使用的最小版本**。

#### 原因 2：桌面是主用方式，必须先守住

你的真实使用方式是桌面优先，所以：

```text
任何新阶段开始前，先确保桌面主路径没有回退，
再继续扩 Web 或 headless 能力。
```

#### 原因 3：自动模式治理属于“重要但不应抢跑”的能力

自动环境检测确实重要，但如果在 Web 最小闭环还没稳定前就大动启动链路，容易把问题面放大。

因此更合理的是：

- 先把桌面 + Web 最小闭环稳住
- 再补自动模式治理
- 这样调试面更小，风险更可控

#### 原因 4：个人系统先要“能管理”，再要“很完美”

个人工具的第一优先级不是平台级完整性，而是：

- 我现在能继续用桌面
- 必要时浏览器能接手核心管理
- 无桌面环境时也不至于失控

这决定了顺序必须偏向“先可用，再补完美”。

---

### 5.2A 执行纠偏与强约束（新增，优先级高于进度描述）

本节用于把本计划中已经出现的偏移方向及时纠正，并将修正后的方案纳入后续执行基线。

#### 纠偏 1：Web Admin 的定位不得漂移为“第二套页面系统”

必须明确：

```text
Web Admin 只是第二个入口，不是第二套页面系统。
```

因此后续实现中：

- 允许新增：Web 路由壳、登录壳、布局壳、运行环境壳
- 禁止新增：与桌面业务功能一一对应的 Web 专用业务页面副本
- 禁止把 `src/web-admin/*` 演化为长期承载业务逻辑与业务组件的第二前端根目录

强约束：

1. `src/web-admin/*` 只允许放：
   - 路由入口
   - 登录鉴权相关壳层
   - Web 容器布局
   - Web 环境专属极薄适配
2. 渠道、设置、Pool、Token、Log、Dashboard 等业务 UI 必须沉淀在 `src/features/*`
3. 若某功能在 Web 端需要新增交互，默认先修改共享 feature；只有明确属于 Web 壳层差异时，才允许留在 `src/web-admin/*`

#### 纠偏 2：禁止“各页面复制改造”式推进

以下思路全部视为偏离方向：

```text
复制一个桌面 Page -> 改成 Web Page -> 以后两边分别维护
```

修正后的唯一允许路径：

```text
共享 Feature UI
  ↑          ↑
桌面壳      Web 壳
```

执行约束：

- Desktop Page 与 Web Route 只负责装配共享 feature，不负责承载长期业务逻辑
- 不允许再出现“先复制页面，后续再收敛”的计划措辞与实现路径
- 若当前已有 Web 页面副本，后续任务优先级应是继续下沉到 feature，而不是继续在副本上迭代

#### 纠偏 2A：当前目标不是“已完成单源翻译器”，而是“从双构建共享态收敛到单源态”

必须明确区分：

```text
当前实际：Desktop 构建 + Web 构建，共享部分 feature
最终目标：不再长期维护独立 Web UI 源，只保留极薄 Web 壳层；共享业务 UI 成为真正唯一来源
```

因此当前纠偏口径是：

1. 不把现状误写成“Desktop UI -> 翻译器 -> Web 端口 已成立”
2. 把 `src/web-admin/*` 明确降级为**过渡壳层**，而不是长期业务前端
3. 后续开发目标不是继续扩写 Web UI，而是持续把 Desktop/Web 两侧业务页面逻辑往共享层压缩
4. 任何新增静态标题/文案/布局调整，必须优先改共享层，禁止优先改独立桌面页或独立 Web 页

#### 纠偏 3：后端 handler / command 不得重新长出业务逻辑

必须明确：

```text
Service = 唯一业务逻辑 + 联动副作用落点
Handler / Command = 协议适配层
```

执行约束：

1. `services/*` 负责：
   - CRUD 业务规则
   - 联动写入
   - tray/proxy/admin lifecycle 等业务写操作后的副作用协调
2. `commands/*` 与 `admin/*handlers.rs` 只负责：
   - 入参解析
   - 调 service
   - 错误映射
   - 响应格式转换
3. 禁止在新增 handler/command 中直接堆积：
   - 多步业务判断
   - 直接 DAO 编排
   - 与另一入口不共享的副作用逻辑

#### 纠偏 4：单一 Binary 策略未闭环前，不得表述为“完全完成”

当前可以确认的是：

- `RuntimeMode` 设计已建立
- Combined / Standalone 主路径已建立
- CLI 独立产品线未引入

但以下缺口仍必须明确保留：

- GUI 初始化失败自动降级 Standalone 的完整闭环
- 无桌面环境下人工验证与日志可观测性兜底

因此执行口径统一为：

```text
单一 Binary 策略：架构已成立，闭环仍需验证，不得提前宣称 100% 完成。
```

#### 纠偏 5：计划文档必须区分“执行基线”和“历史补充/审查材料”

后续不得再把以下内容混写成同一层级：

- 当前执行基线
- 历史过渡方案
- 审查期补充设计
- 已完成进度记录

统一要求：

1. 主计划正文只保留：
   - 目标
   - 约束
   - 阶段顺序
   - DoD
   - 强制规则
2. 历史审查、旧过渡方案、补充实现草案必须明确标记为：
   - 附录
   - 审查记录
   - 历史方案（不作为当前默认实施路径）
3. 若附录与主计划冲突，以主计划正文为准

#### 纠偏 6：阶段进度描述不得反向污染阶段约束

允许实际开发中出现“先完成了后续阶段的一部分”，但计划口径必须保持一致：

- 阶段顺序是治理约束，不因局部先做而失效
- 当前状态表只能描述“实际已完成到哪里”
- 不能因为 Phase 2/3 的局部实现已存在，就削弱 Phase 1 的退出门槛

统一表述要求：

```text
实际实现进度 ≠ 阶段治理顺序。
```

如果某后续阶段内容已提前实现，应写为：

- “已实现基础能力，但未改变当前阶段退出条件”

而不能写成：

- “后续阶段已完成，因此当前阶段可弱化”

---

### 5.2B 执行期硬性 guard rails（新增）

以下约束自本计划修订后立即生效。

#### 前端 guard rails

1. `features/*` 禁止直接依赖：
   - `@tauri-apps/api`
   - `fetch`
   - Web 专属路由对象
2. `features/*` 只能通过 `useApiAdapter()` 或 adapter 注入方式访问后端
3. `src/web-admin/*` 禁止复制下列业务组件形成长期分支：
   - 表格
   - 表单
   - 编辑弹窗
   - 管理器主视图
4. 若新增 Web 特有 UI，只能是壳层级差异，不得复制业务核心组件

#### 后端 guard rails

1. 新增管理能力时，先判断是否需要新增/扩展 `services/*`
2. `commands/*` 与 `admin/*handlers.rs` 禁止直接形成两套业务分支
3. 对同一业务动作，应能建立如下映射：

```text
Desktop command -> same service <- Admin handler
```

4. 所有“写操作后联动”必须可定位到统一复用函数或 service，而不是分散在不同入口各自补逻辑

#### 设置链路 guard rails

1. `updateSettings()` 仍以完整对象更新为主路径
2. 任何 Settings 一致性结论，必须完成：
   - 前端真实传参检查
   - Adapter / IPC / HTTP 序列化检查
   - Rust 反序列化检查
   - DB 写入检查
   - 读回渲染检查
3. `Partial<AppSettings>`、`as any`、字段掩码回填不完整，全部视为高风险信号

#### 文档 guard rails

1. 主计划中禁止再出现“复制页面改 Web 版”式表述
2. 主计划中禁止把历史补充方案写成当前默认路径
3. 所有“已完成”表述必须区分：
   - 架构已成立
   - 代码已落地
   - 人工验证已闭环
4. 未完成人工闭环验证的事项，不得写成“最终完成”

---

### 5.3 对应新的阶段顺序（替代原 A/B/C/D/E 作为执行顺序）

#### Phase 1：桌面主路径加固 + Web 最小闭环收口

优先完成：

1. 桌面 Tray / Channel / Settings / Proxy 主路径回归
2. Web 登录 / Settings / Channels 闭环
3. `channel_service` + `ChannelManager` + `SettingsEditor` 继续收口
4. Web 错误反馈达到“能定位问题”的程度
5. **识别并冻结前端单源差距清单**：
   - 列出所有仍直接依赖 `@tauri-apps/api` / `@/lib/api` 的桌面页面
   - 列出所有尚未下沉到 `src/features/*` 的业务页面逻辑
   - 列出所有仅存在于 `src/web-admin/*` 的业务 UI 逻辑

**这一阶段结束后应达到**：

```text
桌面继续是主用入口；
Web 已经不只是演示壳，而是可完成核心管理；
同时，单源前端差距已经被显式识别，后续收口路径已锁定。
```

#### Phase 2：运行模式治理（第一版可用）

优先完成：

1. `RuntimeMode`
2. env / 参数覆盖
3. 无 GUI -> Standalone
4. 模式日志输出
5. GUI 失败时的基础降级策略

**这一阶段结束后应达到**：

```text
同一个 binary 在桌面和无桌面环境都能以明确方式进入可用模式。
```

#### Phase 3：前端单源收口（新增，必须执行）

优先完成：

1. 桌面端页面去直连化
   - `App.tsx` 改为通过共享 adapter / shared shell 边界获取数据
   - `ApiPoolPage.tsx` 退出长期主路径，功能收敛到 `features/pool/PoolManager.tsx`
   - `TokenPage.tsx`、`LogPage.tsx`、`DashboardPage.tsx` 继续下沉为 shared feature 或改为 shared feature 壳
2. Web 端业务逻辑去壳内化
   - `src/web-admin/*` 仅保留登录、路由、鉴权、布局、启动壳
   - 所有业务标题、文案、布局、页面主体逻辑继续压缩到共享层
3. 静态 UI 一致性专项验证
   - 任意共享标题/文案/布局改动，都必须能做到“改一处，双端构建/刷新体现”
4. 逐步逼近最终单源态
   - 允许保留 Web 壳层入口
   - 不允许继续新增长期独立 Web 业务页
5. **将新增功能按同一规则接入共享层**
   - 以 Translation Relay 为样板：
     - 左侧 Desktop 入口可以存在
     - Web 展示入口可以存在
     - 但业务主视图、类型、adapter、后端状态、验收标准必须统一写入共享路径与计划

**这一阶段结束后应达到**：

```text
前端仍可保留极薄 Web 壳层，
但业务 UI 已基本收敛为单源；
桌面端修改共享静态 UI 后，Web 刷新可体现同一变更；
新增功能也不再绕开共享层另起双端实现。
```

#### Phase 4：个人系统核心管理面扩展与补漏

优先完成：

1. API Pool
2. Access Tokens
3. 基础 Logs
4. Web 代理启停/状态
5. 若 Phase 3 收口中发现遗漏模块，在本阶段完成补齐

**这一阶段结束后应达到**：

```text
Web Admin 在桌面不可用时，已经足够承担个人系统的大多数日常管理操作；
且这些能力继续沿 shared service + shared feature 路径成立。
```

#### Phase 5：体验优化 + Tray 制度化保障 + 一致性专项回归

优先完成：

1. Tray 回归清单
2. Settings / 导航优化
3. 错误提示统一
4. Dashboard / 状态视图补强
5. 双入口一致性专项回归
6. 前端静态 UI 单源回归

**这一阶段结束后应达到**：

```text
项目从“能用”进入“适合长期个人使用”，
并且前端单源约束已能通过回归验证持续守住。
```

#### Phase 6：工程化封口

优先完成：

1. 防分叉规则
2. CI / review 规则
3. 自动化一致性回归
4. 模式可观测性完善
5. 前端单源检查规则（禁止新增直连页面、禁止新增 Web 业务副本）

#### Phase 7：CLI 后置评估

前置条件不变：

- 单一 binary 不变
- Desktop + Web 已稳定
- Standalone 已成立
- 不破坏桌面主体验

---

### 5.4 每阶段的“非目标”也要锁定

为了防止开发次序再次漂移，每个阶段都明确非目标：

#### Phase 1 非目标
- 不追求完整 Dashboard
- 不追求 Logs/Pool/Tokens 全部同时完成
- 不追求自动模式检测做到全平台完美
- 不推进 CLI 形态

#### Phase 2 非目标
- 不大改 Web 视觉架构
- 不同时推进所有业务模块
- 不引入第二个 binary

#### Phase 3 非目标
- 不一次性做复杂报表化能力
- 不把日志系统做成企业级分析平台

#### Phase 4 非目标
- 不把体验优化演变为重写前端框架
- 不破坏前期已经形成的共享边界

#### Phase 5 非目标
- 不再新增一批业务模块后才开始做规则
- 不把工程化封口无限后拖

---

### 5.5 与当前代码进度对应的最近执行顺序

根据第 10 节当前进度判断，最近的真实执行顺序应当是：

1. **先完成 Phase 1 收口**
   - 桌面主路径回归清单
   - Web 最小闭环补稳定
   - Channels / Settings 真正收口

2. **再进入 Phase 2**
   - 自动模式治理第一版
   - Standalone 可用闭环

3. **然后进入 Phase 3**
   - Pool / Tokens / Logs / 代理控制

4. **再进入 Phase 4 / Phase 5**
   - 体验、一致性、工程化封口

5. **最后才评估 Phase 6**
   - CLI 是否需要、是否值得、是否能保持单一 binary

---

## 6. 补充性建议（纳入实施约束）

### 6.1 启动模式可观测性

启动日志必须输出：

- 检测到的环境状态（GUI / headless）
- 最终运行模式（Combined / Standalone）
- 模式来源（默认 / env / 参数覆盖）
- Admin / Proxy 监听地址与端口

### 6.2 设置项语义调整

- `web_admin_enabled` 不再被视为唯一总开关，而是策略输入项
- 在 headless 模式下，如满足最小可用条件，应优先保证 Web Admin 可用
- 继续遵守 `updateSettings()` 完整对象更新规则

### 6.3 Headless 兜底策略

- GUI 初始化失败时优先尝试 Standalone 降级
- 降级后必须明确输出访问地址与失败原因
- 原则是“可用优先，不因 GUI 失败拖死 Web 管理能力”

### 6.4 安全边界

- Standalone 默认仍监听 `127.0.0.1`
- 若未来支持绑定 `0.0.0.0`，必须输出高亮风险提示
- 登录失败限制继续保留

### 6.7 Translation Relay 开发约束（新增）

本节用于把“桌面侧触发自动翻译 -> 转发到 Web Admin -> Web UI 显示结果”纳入正式计划约束，避免它脱离主架构路线单独生长。

#### 功能目标

```text
Desktop 左侧 UI 组件
  -> 触发自动翻译
  -> 把翻译结果转发到 Web Admin 端口
  -> Web UI 显示最近一次结果
```

#### 架构约束

1. 该功能不是单独的 Web 子系统，而是双入口系统中的一条新数据流
2. Desktop 负责触发，Web 负责读取与展示
3. 共享类型必须统一定义在共享层（如 `src/types.ts`）
4. 调用必须经 adapter，不得在共享组件中直接写 `invoke` / `fetch`
5. 后端状态必须有唯一来源，不允许 Desktop 和 Web 各自维护两套“最近翻译结果”

#### 开发分层要求

1. 前端共享层：
   - `src/features/translator/*` 承担业务主视图
2. Desktop 壳层：
   - 只负责左侧导航入口与页面装配
3. Web 壳层：
   - 只负责展示入口与轮询/鉴权壳层装配
4. 后端：
   - 提供统一的翻译/转发命令与读取端点

#### 当前开发状态要求

在后端闭环完成前，统一标记为：

```text
前端骨架已接；后端未完成；联调未完成；构建未验证。
```

禁止在该状态下宣称：

- 功能已完成
- Web 已稳定显示
- 自动翻译链路已可用

#### 当前待完成项

1. Rust 侧共享状态槽位
2. `translate_and_relay` Tauri command
3. `get_translation_relay` Tauri command
4. `GET /admin/translation-relay` HTTP 端点
5. 翻译服务实现策略定稿
6. 构建检查与联调验证

---

## 7. 联动一致性检查模板（必须执行）

每次宣称“Web 与桌面一致”时，必须同时完成：

### 7.1 静态层

- 类型定义一致
- 默认值一致
- UI 选项一致

### 7.2 数据流层

必须端到端追踪：

- 前端真实传参
- Adapter / IPC / HTTP 序列化
- Rust 反序列化
- DB 写入逻辑
- 前端重新读取并渲染

### 7.3 风险层

- `Partial<AppSettings>` 是风险信号
- `as any` 是风险信号
- 任一链路无法确认，就不能给出“一致”结论

---

## 8. 本次完成定义（DoD）

满足以下全部条件，才算完成本次目标：

1. 一个 binary 同时支持桌面环境与无桌面环境运行
2. 有 GUI 环境下自动进入 Combined
3. 无 GUI 环境下自动进入 Standalone
4. 桌面与 Web 共用同一业务逻辑（services）
5. 桌面与 Web 共用同一业务 UI（features）
6. 不需要维护两套功能实现
7. Channels 在桌面与 Web 均可用且行为一致
8. Settings 全量更新链路验证通过
9. 静态 UI 一致性验证通过：共享业务界面的标题、文案、布局、样式调整在共享层修改一次后，Desktop 与 Web 在重新构建并刷新后都能体现相同变化
10. `cargo check`、`pnpm typecheck`、`pnpm build:web-admin` 通过

---

## 9. 当前推荐执行顺序（锁定）

```text
Phase 1：桌面主路径加固 + Web 最小闭环收口
Phase 2：运行模式治理（第一版可用）
Phase 3：个人系统核心管理面扩展
Phase 4：体验优化 + Tray 制度化保障 + 一致性专项回归
Phase 5：工程化封口
Phase 6：CLI 后置评估
```

当前阶段明确要求：

- 不新增第二个 binary
- 不把 CLI 当作先决条件
- 先守住桌面主用体验
- 先把已有 Web 能力收口到稳定可用
- 再补自动模式治理与无桌面闭环

---

## 10. 对照当前代码的开发项目与完成进度（2026-05-03）

本节用于把“规划项”与“当前已开发状态”逐项对照，避免后续重复判断方向是否偏移。

进度标记说明：

- `✅ 已完成`：代码中已明确存在并接入主流程
- `🟡 部分完成`：已有基础或局部落地，但未形成完整闭环
- `⚪ 未开始 / 未闭环`：规划中有要求，但当前代码尚未满足
- `🔴 明确不作为当前主线`：已定性为后置评估，不纳入当前交付

---

### 10.1 总体目标对照

> 说明：本节只描述“当前代码/验证进度”，不改变第 5 节定义的阶段治理顺序。若后续阶段能力已提前落地，只能说明“已有基础”，不能据此降低前序阶段退出门槛。

| 开发项目 | 目标说明 | 当前状态 | 进度 | 说明 |
|---|---|---|---|---|
| Desktop + Web 双管理入口 | 同一程序同时支持桌面与 Web 管理 | 已具备基础能力 | 🟡 部分完成 | 桌面端与 Web Admin 均可工作，Phase 1 退出前需完成 Tray 人工验证 |
| 单一业务逻辑来源 | Command 与 Admin Handler 共用 Service | Channels/Pool/Token/Log 已落地 | ✅ 已完成 | 4 个 Service 已复用，`log_service` 已完成最后迁移 |
| 单一业务 UI | 桌面与 Web 复用同一套 feature UI | Channels/Settings/Pool/Token/Log 已共享 | 🟡 部分完成 | `ChannelManager`、`SettingsEditor`、`PoolManager`、`TokenManager`、`LogViewer` 已共享 |
| 单一 binary | 不拆第二个服务端程序 | 当前符合 | 🟡 架构已成立，闭环待验 | 当前仍是同一程序体系，CLI 独立 binary 未立项；GUI 失败自动降级仍待闭环验证 |
| 自动检测系统环境 | 自动进入 Combined / Standalone | `detect_runtime_mode()` 已实现 | ✅ 已完成 | Phase 2 运行模式治理代码已落地（CLI 参数 + 环境变量 + 自动检测） |
| 无桌面环境可用 | 无 GUI 环境仍能通过 Web 管理 | Standalone 模式可启动 | 🟡 部分完成 | `RuntimeMode` + `ModeSource` + 启动日志已就位，降级策略待人工验证 |
| CLI 独立形态 | 命令式服务运行 | 当前不做主线 | 🔴 后置评估 | 按当前约束，只有同一 binary 前提下才允许最后评估 |

---

### 10.2 运行模式与启动编排对照

| 开发项目 | 当前状态 | 进度 | 代码依据 | 结论 |
|---|---|---|---|---|
| AdminMode 枚举 | 已有 `Disabled / Standalone / Combined` | ✅ 已完成 | `src-tauri/src/admin/mod.rs` | 运行模式概念已落地 |
| 根据配置判断 Admin 模式 | 已实现 | ✅ 已完成 | `admin::should_start_admin()` / `admin_mode()` | 已有配置驱动的模式判断 |
| 环境变量注入 Admin 配置 | 已实现 | ✅ 已完成 | `admin::apply_admin_env()`、`lib.rs` 启动时调用 | 可通过 env 覆盖管理端配置 |
| Combined 单端口合并路由 | 已实现 | ✅ 已完成 | `build_combined_router()`、`ProxyServer::start_with_admin(...)` 调用链 | 单端口集成已接入 |
| Standalone 独立启动 AdminServer | 已实现 | ✅ 已完成 | `start_admin_if_enabled()` | 双端口/独立 Admin 可启动 |
| 设置变更后重启 Admin / Proxy | 已实现 | ✅ 已完成 | `commands/config.rs` | 配置变更后的运行时联动已存在 |
| RuntimeMode 枚举 | 已实现 | ✅ 已完成 | `src-tauri/src/runtime_mode.rs:3-7` | Combined / Standalone 两种模式 |
| ModeSource 枚举 | 已实现 | ✅ 已完成 | `src-tauri/src/runtime_mode.rs:9-14` | Default / CliArg / EnvVar 三种来源 |
| detect_runtime_mode() | 已实现 | ✅ 已完成 | `src-tauri/src/runtime_mode.rs:16-39` | CLI 参数 > 环境变量 > 自动检测 |
| CLI 参数解析 (--headless/--standalone) | 已实现 | ✅ 已完成 | `runtime_mode.rs:18-23` | 命令行覆盖优先级最高 |
| 环境变量检测 (API_SWITCH_HEADLESS/STANDALONE) | 已实现 | ✅ 已完成 | `runtime_mode.rs:26-35` | 环境变量覆盖次优先 |
| 模式日志输出 | 已实现 | ✅ 已完成 | `lib.rs:36` | 启动时输出运行模式与来源 |
| AppState.runtime_mode | 已实现 | ✅ 已完成 | `lib.rs:27` | 运行模式存入应用状态 |
| GUI 失败自动降级 Standalone | 未闭环 | ⚪ 未闭环 | 当前计划已提出，代码未见完整降级链 | 仍需补齐 |
| 模式可观测性（日志/状态） | 已实现 | ✅ 已完成 | `lib.rs:36` 启动日志输出模式+来源 | 统一"模式来源/判定结果"输出已就位 |

**判断**：

```text
Phase 2 运行模式治理已完成（2026-05-04）。
RuntimeMode / ModeSource / detect_runtime_mode() 三件套已落地，
CLI 参数和环境变量覆盖优先级已明确，启动日志已输出模式与来源。
剩余缺口：GUI 失败自动降级 Standalone（需人工桌面环境验证）。
```

---

### 10.3 Web Admin 后端基础设施对照

| 开发项目 | 当前状态 | 进度 | 代码依据 |
|---|---|---|---|
| Admin 路由骨架 | 已实现 | ✅ 已完成 | `src-tauri/src/admin/router.rs` |
| 静态入口 `/admin` | 已实现 | ✅ 已完成 | `admin/router.rs` + `static_files.rs` |
| `/admin/assets/*path` | 已实现 | ✅ 已完成 | `admin/router.rs` |
| `/admin/login` | 已实现 | ✅ 已完成 | `admin/router.rs` + `handlers.rs` |
| `/admin/logout` | 已实现 | ✅ 已完成 | `admin/router.rs` + `handlers.rs` |
| `/admin/health` | 已实现 | ✅ 已完成 | `admin/router.rs` + `handlers.rs` |
| `/admin/status` | 已实现 | ✅ 已完成 | `admin/router.rs` + `handlers.rs` |
| `/admin/audit-logs` | 已实现 | ✅ 已完成 | `admin/router.rs` + `handlers.rs` |
| `/admin/settings` 读写 | 已实现 | ✅ 已完成 | `admin/router.rs` + `handlers.rs` |
| Bearer Token 鉴权 | 已实现 | ✅ 已完成 | `admin/auth.rs` / `require_auth` |
| 登录失败限制 | 已实现 | ✅ 已完成 | `AdminState.login_failures` + 对应 handler 逻辑 |
| 管理端 CORS | 已实现 | ✅ 已完成 | `admin/cors.rs` + router 中 `apply_admin_cors` |
| 标准化 Admin 错误响应 | 已实现 | ✅ 已完成 | `admin/error.rs` |

**结论**：

```text
Web Admin 后端“壳层基础设施”已基本到位，
当前短板不在基础设施，而在模式治理、业务扩展面与一致性回归。
```

---

### 10.4 Channels 业务复用对照

| 开发项目 | 当前状态 | 进度 | 代码依据 |
|---|---|---|---|
| `channel_service` 存在 | 已实现 | ✅ 已完成 | `src-tauri/src/services/channel_service.rs` |
| Tauri command 调用 `channel_service` | 已实现 | ✅ 已完成 | `src-tauri/src/commands/channel.rs` |
| Admin handler 调用 `channel_service` | 已实现 | ✅ 已完成 | `src-tauri/src/admin/channel_handlers.rs` |
| Channels HTTP API 路由 | 已实现 | ✅ 已完成 | `admin/router.rs` 中 `/admin/channels*` |
| ChannelManager 共享 UI | 已实现 | ✅ 已完成 | `src/features/channels/ChannelManager.tsx` |
| Desktop ChannelPage 变壳层 | 已实现 | ✅ 已完成 | `src/pages/ChannelPage.tsx` |
| Web Admin 使用同一 ChannelManager | 已实现 | ✅ 已完成 | `src/web-admin/src/WebAdminApp.tsx` |
| Channel 组件走 Adapter | 已实现 | ✅ 已完成 | `ChannelList.tsx` / `ChannelFormDialog.tsx` / `ModelSelectionDialog.tsx` |
| Channels 作为 Web 默认核心管理价值 | 已实现基础可用 | ✅ 已完成 | WebAdmin 主界面已展示 Channels |

**结论**：

```text
Channels 是当前最接近“规划闭环”的模块，
可以视为 Desktop + Web 单套开发模式的首个成功样板。
```

---

### 10.5 Settings 复用与联动对照

| 开发项目 | 当前状态 | 进度 | 代码依据 |
|---|---|---|---|
| SettingsEditor 共享组件 | 已实现 | ✅ 已完成 | `src/features/settings/SettingsEditor.tsx` |
| Desktop SettingsPage 复用共享组件 | 已实现 | ✅ 已完成 | `src/pages/SettingsPage.tsx` |
| Web Admin Settings 复用共享组件 | 已实现 | ✅ 已完成 | `src/web-admin/src/WebAdminApp.tsx` |
| Web Admin 设置保存 | 已实现 | ✅ 已完成 | `WebAdminApp.tsx` + `src/web-admin/src/api.ts` |
| 设置更新后 Proxy/Admin 联动重启 | 已实现 | ✅ 已完成 | `commands/config.rs` |
| 完整对象更新约束 | 已在实现中遵守 | 🟡 部分完成 | Web 端保存使用完整 settings 对象，但仍需持续作为红线检查 |
| PATCH/部分更新语义 | 尚未正式建立为主方案 | ⚪ 未开始 / 未闭环 | 当前仍以完整对象更新为主 |

**结论**：

```text
Settings 已完成共享试点，
但它的价值主要是验证“共享组件 + 全量设置更新链路”，
不是最终 Web Admin 主战场。
```

---

### 10.6 前端统一调用层对照

| 开发项目 | 当前状态 | 进度 | 代码依据 |
|---|---|---|---|
| `ApiAdapter` 接口 | 已实现 | ✅ 已完成 | `src/lib/apiAdapter.ts` |
| `tauriApiAdapter` | 已实现 | ✅ 已完成 | `src/lib/tauriApiAdapter.ts` |
| `webAdminApiAdapter` | 已实现 | ✅ 已完成 | `src/lib/webAdminApiAdapter.ts` |
| `useApiAdapter()` 自动切换 | 已实现 | ✅ 已完成 | `src/lib/useApiAdapter.ts` |
| 业务组件经 adapter 调用 | Channels / Pool / Tokens / Dashboard 等已部分实现 | 🟡 部分完成 | `features/channels/*`、`features/pool/PoolManager.tsx`、`features/tokens/TokenManager.tsx`、`features/dashboard/DashboardView.tsx` |
| 全项目范围禁止直连 invoke/fetch | 还未工程化封口 | 🟡 部分完成 | 已有共享实践，但缺少 CI/规则硬限制 |
| 桌面页面仍直连 `@tauri-apps/api` / `@/lib/api` | 明确存在偏移 | 🔴 待收口 | `App.tsx`、`ApiPoolPage.tsx`、`TokenPage.tsx`、`LogPage.tsx`、`DashboardPage.tsx`、`CliPage.tsx` |
| Web 壳层仍持有独立入口与 API 层 | 过渡态存在 | 🟡 可接受过渡态 | `src/web-admin/src/WebAdminApp.tsx`、`src/web-admin/src/api.ts`、`src/web-admin/src/main.tsx` |

**结论**：

```text
前端统一调用层已经成型，但单源目标尚未完成。
当前处于“共享能力已建立、桌面专用页与 Web 壳层仍待压缩”的过渡态。
下一步重点不是新增 adapter，而是消灭桌面页直连、收缩 Web 壳层业务逻辑，并建立单源检查规则。
```

---

### 10.7 桌面主体验 / Tray 对照

| 开发项目 | 当前状态 | 进度 | 代码依据 |
|---|---|---|---|
| Tray 存在且仍是桌面核心能力 | 已实现 | ✅ 已完成 | `src-tauri/src/lib.rs` |
| 渠道/设置变更后 Tray 刷新 | 已实现 | ✅ 已完成 | `commands/config.rs`、`commands/channel.rs`、`commands/pool.rs`、`channel_service.rs` |
| Tray 仅桌面存在 | 当前事实如此 | ✅ 已完成 | Web/Admin 不依赖 tray |
| Standalone 下 tray no-op | 部分具备基础 | 🟡 部分完成 | 已有 `Option<AppHandle>` 思路，但仍需统一收口 |
| Tray 桌面回归门禁 | 尚未制度化 | ⚪ 未开始 / 未闭环 | 计划应补齐为强制验收项 |

**结论**：

```text
Tray 功能当前仍在，
但“Tray 是 Desktop 主体验、必须优先保障”这一点，
还需要在计划与回归流程中被正式制度化。
```

---

### 10.8 其他业务模块迁移对照

| 业务模块 | 目标状态 | 当前进度 | 结论 |
|---|---|---|---|
| API Pool | Service + HTTP + Shared UI | ✅ 已完成 | `pool_service.rs` + `pool_handlers.rs` + `PoolManager.tsx` 三层就位 |
| Access Tokens | Service + HTTP + Shared UI | ✅ 已完成 | `token_service.rs` + `token_handlers.rs` + `TokenManager.tsx` 三层就位 |
| Usage Logs | Service + HTTP + Shared UI | ✅ 已完成 | `log_service.rs` + `usage_handlers.rs` + `LogViewer.tsx` 三层就位（2026-05-05 完成重构） |
| Dashboard | Service + HTTP + Shared UI | ✅ 已完成 | 复用 `log_service.rs` + `usage_handlers.rs` + `DashboardView.tsx` |

**判断**：

```text
Phase 3 核心管理面扩展已基本完成（2026-05-05）。
Channel / Pool / Token / Log / Dashboard 五个模块均已完成 Service + HTTP + Shared UI 三层。
剩余工作：端到端功能验证（Web 冒烟测试）、体验优化与一致性回归（Phase 4）。
```

---

### 10.9 当前总体进度判断

> 说明：本节是“当前实现进度观察”，不是新的阶段顺序定义。即使 Phase 2 / Phase 3 中已有部分能力提前落地，当前仍必须先满足 Phase 1 的退出门槛，之后才算阶段性收口成立。

按“基本能用”和“最终目标”分开估算，避免把后期扩展项混入当前可用性判断。

#### 基本能用进度（Phase 1）

| 大项 | 进度判断 | 说明 |
|---|---:|---|
| 桌面主路径不退化 | 92% | 构建与 Rust 检查通过；DEV 主窗口可见；9098 Proxy/API 与 9099 Web Admin 均归属当前 DEV；托盘交互最后确认 |
| Web Admin 基础设施 | 95% | 登录、鉴权、静态入口、状态、Settings、Channels、静态资源与 shared shell 已通过浏览器验证 |
| Channels 共享闭环 | 98% | CRUD、probe、selectModels、selected_models 与 api_entries 写库/清理已通过 DEV API + DB 验证 |
| Settings 全量更新链路 | 98% | `{ data, _version }` envelope、版本递增、409 冲突、端口迁移/恢复 restart contract 已验证 |
| Web 可见错误反馈 | 85% | 登录/Channels/probe/Settings 关键错误有结构化反馈；toast 统一仍是后续体验项 |
| 构建/类型/后端检查 | 95% | `cargo check`、`pnpm typecheck`、`pnpm build:web-admin`、`pnpm build:renderer` 已通过；仍有非阻断 warning/chunk warning |
| Phase 1 基本可用完成度 | **约 92% ~ 95%** | 显示一致性已优先收口；操作一致性核心链路已验证，剩余主要是桌面 tray/window 人工冒烟与后续体验统一 |

#### 总体目标进度（含后续阶段）

| 大项 | 进度判断 | 说明 |
|---|---:|---|
| Web Admin 基础设施 | 95% | 基础可用，9099 浏览器验证通过，静态资源与 shared shell 显示一致性已优先收口 |
| Channels 共享闭环 | 98% | CRUD、probe、selectModels、selected_models 与 api_entries 已通过 DEV API + DB 验证 |
| Settings 共享试点 | 98% | 共享 UI + 全量更新链路、版本冲突、端口迁移 restart contract 已验证 |
| 自动环境检测与统一模式治理 | 95% ✅ | Phase 2 已完成：RuntimeMode / ModeSource / detect_runtime_mode() / CLI+env 覆盖 / 启动日志 |
| Standalone/headless 闭环 | 80% | 运行模式检测已完成，GUI 失败降级策略待人工验证 |
| Tray 桌面优先保障制度化 | 70% | 代码入口已收口；托盘放在显示一致和操作一致之后最后确认 |
| 其余业务模块共享迁移 | 85% | Pool / Token / Log / Dashboard Service+HTTP+UI 三层已就位，待端到端冒烟验证 |
| 总体目标完成度 | **约 85%** | Phase 2+3 已完成主体；剩余 Phase 1 退出验证 + Phase 4 体验优化 + 一致性回归 |

### 10.10 离“基本能用”还差什么

按个人系统的 Phase 1 标准，离“基本能用”已经不远，剩余主要不是大架构问题，而是收口验证：

1. **显示一致性优先（当前已基本完成）**
   - Web 登录
   - Web 主界面使用共享 `MainShell`
   - Web 与桌面主导航样式、间距、边框、配色基本一致
   - public 静态资源（star/favicon/logo）在 `/admin` 下正确显示
   - Web theme / locale 从 settings 应用，与桌面链路对齐

2. **操作一致性继续收口（当前核心链路已验证，9098/9099 主次已确认）**
   - 9098 = DEV Proxy/API 主服务端口
   - 9099 = DEV Web Admin 管理端口
   - Web Channels CRUD
   - Web Settings 保存
   - Web selectModels 后 API Pool 是否同步

3. **托盘最后确认（可押后，不阻塞显示一致性）**
   - 桌面启动 / 主窗口打开
   - Tray 打开 / Tray 刷新
   - 桌面 UI Proxy 启停

4. **Web 最小闭环边界检查**
   - 登录过期/401 后是否能回登录页
   - Settings 版本冲突是否能显示清楚
   - Channels 错误是否能看到明确提示

3. **文档进度同步**
   - 把 Phase 1 已完成项正式标记
   - 把 Phase 2 的启动条件写清楚

因此当前判断：

```text
按“先显示一致，再操作一致”的新顺序，
显示一致性已经基本完成；
9098（Proxy/API）与 9099（Web Admin）的 DEV 主次已确认；
Phase 1 离“基本能用”大约还差 3% ~ 5%，
主要是托盘真实交互和桌面 UI 细节，可按优先级押后；
离“完整规划目标”还差约 30%，
主要是 Phase 2 运行模式治理与 Phase 3 其他业务模块迁移。
```

### 10.11 当前阶段结论

当前最准确的结论应是：

```text
项目已经完成 Web Admin 底层基础设施，
并在 Channels / Settings 上验证了“Desktop + Web 单套开发”的路线可行；
Phase 1 已经完成“显示一致性优先收口”，Web Admin 与桌面端在共享 MainShell、基础 CSS token、主题/语言和静态资源上已对齐；
Channels / Settings 的核心操作链路已经通过 DEV API + DB 验证；
下一步应继续补操作一致性中仍未覆盖的少量 UI 路径；
托盘作为最后确认项，不阻塞当前 Web 显示一致性和核心操作一致性判断，
通过后再进入运行模式治理与无桌面环境闭环。
```

### 10.12 下一步优先顺序（根据当前真实进度重排）

1. **先完成并确认 Phase 1 退出门槛**
   - 锁定桌面主路径人工冒烟
   - 锁定 Web 最小闭环人工冒烟
   - 锁定 Channels / Settings / 关键联动一致性

2. **对已提前落地的 Phase 2 能力只做闭环验证，不改变阶段治理顺序**
   - `detect_runtime_mode()` 已有代码基础
   - 当前重点是补人工验证、日志验证、GUI 失败降级闭环

3. **对已提前落地的 Phase 3 能力只做冒烟和收口，不视为已完成整个阶段**
   - Pool / Tokens / Logs / Dashboard 先验证共享路径是否稳定
   - 若发现分叉实现，优先继续下沉到 service / feature

4. **托盘最后确认（可与上述收口并行，但不得跳过）**
   - tray 图标 / 菜单 / 排序刷新
   - Web 写操作后的 tray 联动一致性

5. **在 Phase 1 真正通过后，再正式宣布进入后续阶段**
   - Phase 2：运行模式治理闭环
   - Phase 3：个人系统核心管理面完整收口
   - Phase 4/5：体验、一致性、工程化封口

6. **最后才评估是否需要同一 binary 下的 CLI 形态**

---

### 10.13 Phase 1 当前已完成项（截至 2026-05-05）

以下内容已可视为 Phase 1 收口成果：

#### 已完成

- ✅ Web Admin 基础设施可构建、可运行
- ✅ `/admin` / `/admin/login` / `/admin/settings` / `/admin/channels*` 主链路已存在
- ✅ 登录后的 Web Main 已取消临时壳，改为映射共享 `MainShell`
- ✅ Desktop / Web 已共享主 Main 的外壳结构、导航结构与基础 CSS token
- ✅ `ChannelManager` 已作为 Desktop + Web 共享渠道 UI
- ✅ `SettingsEditor` 已作为 Desktop + Web 共享设置 UI
- ✅ `channel_service` 已作为 Channels 唯一业务逻辑来源
- ✅ Tray 刷新逻辑已收口到统一入口 `refresh_tray_if_enabled()`
- ✅ `selectModels` 双入口数据流已修正为一致路径
- ✅ Web Channels 组件已补基础错误反馈
- ✅ Web Settings 更新已改为完整对象 + `_version` envelope
- ✅ Web Settings 读取已补 `_version` 回填，避免版本号丢失
- ✅ Web Admin session 已调整为 24 小时滑动续期，方便 dev 冒烟
- ✅ `cargo check` 通过
- ✅ `pnpm typecheck` 通过
- ✅ `pnpm build:web-admin` 通过
- ✅ `pnpm build:renderer` 通过
- ✅ Pool toggle 发送格式修正（布尔值 vs 对象）
- ✅ Pool 一键开关/置顶键盘事件支持
- ✅ Channel 表格延迟列显示
- ✅ Settings Web 环境隐藏（Web 管理、系统启动、启动最小化）
- ✅ API 增加保存报错修正（camelCase → snake_case）
- ✅ 静态资源可访问（star.jpg 等）
- ✅ MainShell 共享 UI 已实现
- ✅ 后端 HTTP 端点完整实现（proxy/status/start/stop, test-chat）
- ✅ Adapter 接口扩展（settings, proxy, testChat）
- ✅ Phase 2 运行模式治理完成：`RuntimeMode` + `ModeSource` + `detect_runtime_mode()` + CLI/env 覆盖 + 启动日志（`runtime_mode.rs`）
- ✅ Phase 2 `AppState.runtime_mode` 已注入（`lib.rs:27`）
- ✅ Phase 3 `pool_service.rs` 创建完成，Pool 三层（Service + HTTP + UI）已就位
- ✅ Phase 3 `token_service.rs` 创建完成，Token 三层已就位
- ✅ Phase 3 `log_service.rs` 创建完成（9 函数），Log 三层已就位
- ✅ Phase 3 `usage_handlers.rs` 重构完成（8/8 handler 调用 `log_service`）
- ✅ Phase 3 `commands/usage.rs` 重构完成（8/8 command 调用 `log_service`）
- ✅ Phase 3 Dashboard 复用 `log_service`，三层已就位

#### 尚未完成（Phase 1 退出前必须确认）

- ⏳ 桌面侧操作体验与 Web 操作体验的最终人工对照
- ⏳ 托盘最后确认：tray 图标 / tray 菜单 / tray 排序刷新
- ⏳ Phase 1/2/3 退出后进入 Phase 4（体验优化 + 一致性回归）

### 10.14 Phase 1 冒烟验证清单（退出门槛）

以下清单通过后，才认为 Phase 1 达到“基本能用”。

#### A. 桌面主路径冒烟

1. 程序桌面启动成功
2. 主窗口正常打开
3. Tray 正常显示
4. Tray 菜单可打开
5. 点击 tray 模型项后排序变更，且菜单刷新
6. Channel 页面可进入
7. Settings 页面可进入
8. Proxy 可启动
9. Proxy 可停止
10. 修改排序模式/设置后 tray 可刷新

#### B. Web Admin 最小闭环冒烟

1. `/admin` 可访问
2. 登录成功
3. 错误登录提示可见
4. `GET /admin/settings` 成功
5. Settings 保存成功
6. Channels 列表成功
7. 创建 Channel 成功
8. 编辑 Channel 成功
9. 删除 Channel 成功
10. `probe_url` 可返回结果
11. `fetch_models` 可返回结果或明确错误
12. `selectModels` 保存成功
13. 登录后主界面使用共享 `MainShell`，不再显示临时 Web Main
14. 主导航样式、间距、边框、配色与桌面主 Main 基本一致
15. `star.jpg` / logo 等 public 资源在 `/admin` 下可显示

#### C. 联动一致性冒烟

1. Web 保存 Settings 后，再次读取版本号递增
2. Web 保存 Settings 后桌面端不出现明显回退
3. Web `selectModels` 后 `selected_models` 更新
4. Web `selectModels` 后 `api_entries` 同步更新
5. Web 改排序或关键状态后桌面 Tray 保持一致
6. Web 与桌面在相同 settings 下 theme / locale 显示一致

#### D. 静态 UI 一致性冒烟（新增，硬门槛）

1. 修改共享页面标题后，Desktop 构建产物显示新标题
2. 在不单独修改 Web 壳层业务内容的前提下，Web 构建并刷新后显示相同标题
3. 修改共享按钮文案后，Desktop 与 Web 构建结果显示相同文案
4. 修改共享列表/表单布局后，Desktop 与 Web 构建结果显示相同布局
5. 若任一变更需要在 Web 端再做第二次等价修改，判定为 UI 分叉，Phase 1 不通过

#### E. 构建与检查门槛

1. `cargo check` 通过 ✅
2. `pnpm typecheck` 通过 ✅
3. `pnpm build:web-admin` 通过 ✅
4. `pnpm build:renderer` 通过 ✅

### 10.16 前端单源差距清单（新增）

本节用于明确“当前实际架构”与“目标单源前端”之间的具体差距，作为后续收口依据。

#### 当前实际架构

```text
共享 features / shared shell / shared tokens
        ↑                     ↑
Desktop 独立构建           Web 独立构建
(src/pages, App.tsx)       (src/web-admin/*)
```

#### 目标架构

```text
共享业务 UI / 共享静态文案 / 共享布局 / 共享样式
        ↑                     ↑
Desktop 壳                 Web 壳
```

即：允许存在极薄双壳层，但不允许长期存在双业务前端源。

#### 当前主要差距

1. **Web 端仍存在独立前端入口与独立 API 层**
   - `src/web-admin/index.html`
   - `src/web-admin/src/main.tsx`
   - `src/web-admin/src/WebAdminApp.tsx`
   - `src/web-admin/src/api.ts`
   - 判断：目前可接受为过渡壳层，但不得继续向其中沉积业务页面逻辑

2. **Desktop 端仍有多个页面直连 Tauri/API 层**
   - `src/App.tsx` 直连 `@tauri-apps/api/app` 与 `@/lib/api`
   - `src/pages/ApiPoolPage.tsx` 直连 `@tauri-apps/api/event` 与 `@/lib/api`
   - `src/pages/TokenPage.tsx` 直连 `@/lib/api`
   - `src/pages/LogPage.tsx` 直连 `@tauri-apps/api/event` 与 `@/lib/api`
   - `src/pages/DashboardPage.tsx` 直连 `@/lib/api`
   - `src/pages/CliPage.tsx` 直连 `@/lib/api`
   - 判断：这些页面说明前端单源目标尚未成立，必须持续下沉或改壳

3. **共享 feature 覆盖面已建立但未完全接管桌面主路径**
   - 已共享：Channels、Settings、PoolManager、TokenManager、LogViewer、DashboardView、MainShell
   - 未完全共享：桌面实际入口仍未全部切到 shared feature 路径

4. **静态 UI 一致性在部分模块具备基础，但全局保障不足**
   - 共享模块中改标题/文案/布局，双端可同步构建体现
   - 直连桌面页中的静态 UI 改动，仍可能无法自动映射到 Web

#### 纠正后的开发要求

1. 以后新增功能默认先落 `src/features/*`
2. 现有桌面页面如已存在 shared feature，优先改为薄壳或退出主路径
3. `src/web-admin/*` 不再新增业务页面主体，只允许壳层与鉴权/路由装配
4. 所有“桌面改一处静态 UI，Web 刷新也体现”的验收，必须基于共享层成立，而不是基于巧合或重复修改

#### 当前阶段的直接行动项

- 把 `ApiPoolPage.tsx` 从长期主路径收缩为过渡页，能力收口到 `features/pool/PoolManager.tsx`
- 让 `TokenPage.tsx`、`LogPage.tsx`、`DashboardPage.tsx` 逐步退化为 shared feature 壳
- 审查 `App.tsx`，把可共享的数据加载与页面装配继续压向 shared shell / adapter 边界
- 对 `src/web-admin/src/WebAdminApp.tsx` 设定明确上限：只保留登录、壳层、启动装配，不再扩写业务逻辑

### 10.17 Translation Relay 当前开发清单（新增）

| 项目 | 当前状态 | 进度 | 说明 |
|---|---|---|---|
| 共享功能文件 `src/features/translator/TranslationRelayPanel.tsx` | 已开始创建 | 🟡 前端骨架 | 包含 Desktop 发送面板与 Web 展示视图思路 |
| 左侧导航接线 | 已开始 | 🟡 前端骨架 | `MainShell.tsx` / `App.tsx` 已开始加入 translator 页面分支 |
| Web 展示接线 | 已开始 | 🟡 前端骨架 | `WebAdminApp.tsx` 已开始加入 translator 读取与展示思路 |
| 共享类型定义 | 已开始 | 🟡 前端骨架 | `TranslationRelayPayload` / `TranslationRelayRequest` 已进入计划开发内容 |
| Adapter 接口扩展 | 已开始 | 🟡 前端骨架 | Desktop / Web adapter 已开始扩展 translation 接口 |
| Rust 后端状态 | 未完成 | ⚪ 未闭环 | 尚未形成最近翻译结果的统一状态槽位 |
| Tauri commands | 未完成 | ⚪ 未闭环 | `translate_and_relay` / `get_translation_relay` 仍待实现 |
| Admin HTTP 端点 | 未完成 | ⚪ 未闭环 | `/admin/translation-relay` 仍待实现 |
| 自动翻译实现策略 | 未定稿 | ⚪ 未闭环 | 仍需在外部翻译 API / 现有模型翻译 / 其他方案中确定 |
| 构建与联调验证 | 未完成 | ⚪ 未闭环 | 仍需 `pnpm typecheck`、`pnpm build:web-admin`、`pnpm build:renderer`、`cargo check` |

**当前结论**：

```text
Translation Relay 已进入开发中，
但当前只能视为“前端骨架已接”的功能，
不能视为已完成能力。
```


### 11.0 第一原则重申：先转换到“基本能跑起来”

当前阶段最重要的不是继续追求规划完整度，而是先把系统转换到：

```text
Desktop + Web 两个入口都能基本跑起来。
```

这里的“基本能跑起来”不是抽象说法，而是指：

#### 桌面侧

- 程序能启动
- 主窗口能打开
- Tray 能显示
- Proxy 能启停
- Settings 能保存
- Channels 页面能操作

#### Web 侧

- `/admin` 能访问
- 能登录
- 能读 settings
- 能保存 settings
- 能看 channels
- 能做 channels 最基本 CRUD

#### 联动侧

- 不出现一改设置就把系统打坏
- 不出现 Web 保存后桌面明显异常
- 不出现 `selectModels` 只写一半数据

因此，Phase 1 的第一优先级必须理解为：

```text
先跑起来，再跑稳一点，最后再补漂亮和完善。
```

### 11.0.1 Phase 1 的内部顺序再收紧

为了和“先跑起来”的目标一致，Phase 1 再拆为三个内部步骤：

#### Phase 1A：先跑起来

只先保证：

1. Desktop 能启动
2. Web Admin 能打开
3. 登录可用
4. Settings 可读写
5. Channels 可 CRUD
6. 基础构建检查通过

#### Phase 1B：再跑稳一点

在能跑起来之后，再补：

1. Tray 刷新收口
2. Channels 错误提示可见
3. `selectModels` 数据流一致
4. Settings `_version` 链路正确

#### Phase 1C：退出前确认

最后再做：

1. 桌面冒烟
2. Web 冒烟
3. 联动冒烟
4. 文档打标

这个顺序的意义是：

```text
先确认“跑起来”是否成立，
再确认“收口得是否漂亮”。
```

本轮已完成的工作，主要属于 **Phase 1B：再跑稳一点**。

本轮之后，下一步应优先进入 **Phase 1C：退出前确认**。

### 11.1 分期总原则

#### 原则 1：桌面主路径优先

桌面是主要使用方式，因此：

- Tray
- 窗口体验
- 本地设置修改
- 本地渠道管理
- 本地代理启停

这些都属于第一优先级，不能因为 Web Admin 改造而退化。

#### 原则 2：Web 先解决“能管理”，再解决“很完善”

Web Admin 第一阶段只要求：

- 能登录
- 能看状态
- 能改核心设置
- 能管理 Channels
- 在无桌面环境中能替代桌面完成最基本管理

不要求第一阶段就把所有业务、全部视觉、全部细节都做到桌面等价。

#### 原则 3：先保留简单实现，只要不破坏总架构

有些能力可以先简单实现，只要满足：

- 不引入第二套业务逻辑
- 不破坏单一 binary 约束
- 不让后续补强时必须推倒重来

例如：

- 自动环境检测第一版可先用 env/参数覆盖 + 保守默认策略
- Standalone 第一版可先做到“后端可启动、Web 可登录、Channels 可管理”
- Web 页面结构第一版可先保持简单 Tabs，而不是一次性完整后台框架

#### 原则 4：每个“先简单实现”的点，都必须带后续拆分步骤

禁止只写：

```text
先这么做，后面再优化
```

必须写成：

```text
第一版怎么做
第二版补什么
第三版如何收口
```

这样后续才不会因为“临时方案失控”再次大改。

---

### 11.2 个人系统视角下的功能优先级重排

#### P0：今天不用就难受的能力（必须先完成）

1. 桌面端原有主流程不退化
2. Tray 保持完整可用
3. Web Admin 至少能管理 Channels
4. 设置修改后程序行为正确联动
5. 有桌面 / 无桌面环境都能启动到“可管理状态”

#### P1：个人系统高频管理能力（第二阶段完成）

1. API Pool 管理
2. Access Tokens 管理
3. 基础日志查看
4. 基础运行状态查看
5. Web 端代理启停与状态反馈
6. **模型分组** — `auto` 为默认分组，支持自定义分组（如 `coding`、`translate`）。每个 entry 唯一归属一个分组。请求路由：分组精确匹配 > 模型名子串匹配（≥5 字符）> auto 兜底。Tray 菜单增加分组切换。详见 `model-grouping-plan.md`

#### P2：个人系统体验增强（第三阶段完成）

1. Dashboard
2. 更完整的审计/日志视图
3. 更好的错误提示
4. 设置页体验优化
5. Web 信息架构优化

#### P3：长期维护与附加运行方式（最后完成）

1. 防分叉工程化规则
2. 自动化一致性回归
3. 更稳的 headless 自动检测
4. 同一 binary 下 CLI 评估

---

### 11.3 新分期方案（面向个人系统）

## Phase 1：保桌面、通 Web、先可用

### 目标

先保证个人桌面主路径完全不退化，同时把 Web Admin 做到“最低可用闭环”。

### 本期必须完成

1. **桌面主路径不退化**
   - Tray 继续可用
   - Settings 继续可用
   - Channel 页面继续可用
   - Proxy 启停继续可用

2. **Web Admin 最小闭环**
   - 登录
   - 状态查看
   - Settings 读取/保存
   - Channels 列表/新增/编辑/删除
   - fetch_models / select_models / probe_url 基本可用

3. **统一复用路径成立**
   - `channel_service` 继续作为唯一渠道业务来源
   - `ChannelManager` 继续作为唯一渠道 UI
   - `SettingsEditor` 继续作为共享设置 UI

4. **运行模式先做到“能用”**
   - 第一版允许优先采用保守策略：
     - GUI 环境：按当前桌面流程启动
     - 无 GUI / 明确 env 覆盖：进入 Standalone
   - 不要求第一版就做到所有平台 100% 智能检测，但必须保证有明确兜底入口

### 本期允许简单实现的点

1. **自动环境检测第一版可简化**
   - 先支持环境变量 / 参数覆盖
   - 平台 GUI 自动检测先做保守实现
   - 文档写明哪些平台场景后续继续补强

2. **Web 布局先简单**
   - 可先保留当前 `WebAdminApp` 的简单 tabs / shell
   - 不强求第一阶段做完整后台路由框架

3. **错误提示先保证可见**
   - 第一版允许用较简单的 message 展示
   - 后续再统一 toast / 错误码映射

### 本期验收标准

- 桌面使用者不感觉原功能被 Web 改造破坏
- Web Admin 能完成最关键的 Channels 管理
- 设置更新链路正确
- 无桌面环境下至少可以登录 Web 管理端并进行基础操作

### 本期完成后的状态定义

```text
个人用户已经可以继续以桌面为主使用；
需要时也可以通过浏览器完成核心管理；
即使在无桌面环境中，也不至于完全无法管理。
```

---

## Phase 2：补齐个人系统的核心管理面

### 目标

在最小可用闭环成立后，把个人系统真正高频需要的管理能力迁移到共享模式。

### 本期重点

1. **API Pool 迁移**
   - Service 层抽离/确认
   - HTTP API 接入
   - PoolManager 共享 UI

2. **Access Tokens 迁移**
   - 列表 / 创建 / 删除 / 启停
   - 共享 TokenManager

3. **基础 Logs 迁移**
   - 先做可读、可筛选、可查看详情
   - 不要求第一版做非常复杂的统计分析

4. **Web 代理控制补齐**
   - 启停
   - 当前状态
   - 基础错误反馈

### 本期允许简单实现的点

1. 日志页可先做“够看”版本
   - 先支持分页、状态、错误信息、时间
   - 后续再补更复杂的聚合视图

2. Token 管理先保基础 CRUD
   - 复制体验、批量功能等可后置

3. Pool 页面先保功能一致
   - 视觉与微交互可晚一版再细抠

### 本期验收标准

- 不依赖桌面窗口，也能完成个人系统最核心管理动作
- Pool / Tokens / Logs 至少具备可用版本
- 这些模块开始进入“同一 service + 同一 feature”轨道

### 本期完成后的状态定义

```text
Web Admin 已不再只是补充入口，
而是个人系统在桌面不可用时的可靠备用管理端。
```

---

## Phase 3：桌面优先前提下补体验与一致性

### 目标

在功能可用后，补齐个人使用过程中的体验问题与一致性问题。

### 本期重点

1. **Tray 保障制度化**
   - 建立 tray 回归清单
   - 所有关键写操作后的 tray 刷新行为逐项验证

2. **Settings / Web 信息架构优化**
   - 梳理设置分区
   - 优化 Web Admin 导航
   - 降低个人使用时的操作成本

3. **错误处理与反馈优化**
   - 统一错误响应映射
   - 前端统一提示方式
   - 长操作超时提示更清晰

4. **Dashboard / 状态视图补强**
   - 提供更适合个人系统观察的视图
   - 不追求企业报表化，重在实用

5. **联动一致性专项回归**
   - Channels
   - Settings
   - Pool
   - Tokens
   - Logs

### 本期允许简单实现的点

1. Dashboard 可以先偏“运行观察”而非“高级分析”
2. 审计日志可以先维持基础可读，不做复杂检索
3. 通知组件可以先统一，但不必一次性做完整设计系统

### 本期验收标准

- 桌面主体验稳定
- Web 端使用成本明显下降
- 常见报错能被正确提示
- 双入口核心数据流一致性可验证

### 本期完成后的状态定义

```text
项目从“能用”进入“适合长期个人使用”的阶段。
```

---

## Phase 4：工程化封口，防止后续再次分叉

### 目标

把当前已验证可行的双入口统一方案，转化为长期不会跑偏的工程规则。

### 本期重点

1. **前端防分叉规则**
   - `features/*` 禁止直连 `invoke/fetch`
   - 统一走 `useApiAdapter()`

2. **后端防分叉规则**
   - 新管理能力必须先入 `services/*`
   - command / handler 只做适配

3. **一致性回归用例**
   - 同用例跑桌面入口与 Web 入口
   - 对比 DB 与返回结果

4. **模式可观测性补齐**
   - 统一日志输出
   - 统一状态查看入口

### 本期验收标准

- 新需求默认不会走向“复制一份 Web 版”
- 团队后续开发时有明确 guard rails
- 模式判断、运行状态、主要错误都更容易排查

### 本期完成后的状态定义

```text
架构已经收口，
后续新增功能的成本主要体现在业务本身，
而不是再次处理桌面/Web 分叉问题。
```

---

## Phase 5：后置补强项（最后评估）

### 目标

只在前 4 个阶段已经稳定后，再评估是否需要继续补强附加运行方式与更复杂能力。

### 本期候选项

1. 更稳的跨平台 headless 自动检测
2. 更完整的运行模式覆盖策略
3. 更精细的日志/审计/诊断能力
4. **同一 binary 下 CLI 评估**

### CLI 评估前提（再次强调）

只有同时满足以下条件才允许进入：

1. Desktop + Web 已稳定
2. 无桌面环境已能通过同一程序运行 Standalone
3. 不需要拆第二个 binary
4. 不破坏当前桌面主体验

### 本期结论原则

```text
CLI 不是为了“看起来更完整”而做，
而是只有在个人系统长期使用中确实有价值，
且不破坏单一 binary 底线时才做。
```

---

### 11.4 “先简单实现”与“后续补强”的拆分模板

后续每个子功能都按以下模板写，避免再次返工：

#### 模板

**第一版（先能用）**
- 最小可用范围
- 明确允许的简化点
- 不允许触碰的架构红线

**第二版（补高频体验）**
- 个人使用中最常见痛点
- 对应补强项

**第三版（工程化收口）**
- 防分叉规则
- 自动化回归
- 可观测性

#### 示例：自动环境检测

**第一版**：
- env / 参数覆盖优先
- 默认采用保守策略
- 能明确进入 Combined 或 Standalone

**第二版**：
- 增强不同平台 GUI 可用性检测
- 补齐 GUI 初始化失败后的降级链

**第三版**：
- 模式判定日志标准化
- 增加统一状态查看能力
- 增加回归测试覆盖

#### 示例：Web Admin 页面结构

**第一版**：
- 简单 shell + tabs / 少量页面
- 先保功能可达

**第二版**：
- 优化导航与信息架构
- 改善个人系统高频操作路径

**第三版**：
- 建立统一页面骨架规范
- 固化组件复用规则

---

### 11.5 该重新分期方案的核心价值

```text
它不是把复杂问题往后拖，
而是先把个人用户最需要、最常用、最不能退化的能力稳住；
同时把每个“临时简单方案”的后续补强路径提前写清楚，
避免未来再做结构性大改。
```

---

### 12. 当前执行基线（整理后锁定）

本节用于把前文分散约束整理成当前唯一执行基线，避免后续再被进度描述、历史补充或局部实现路径带偏。

### 12.1 一句话目标

```text
Web Admin 是 API Switch 的第二个入口，不是第二套业务系统。
后续开发必须围绕：后端 Service 复用 + 前端 Feature UI 复用 + Adapter 双实现 + 单一 Binary 运行策略展开。
```

### 12.2 当前不可偏离的 6 条主约束

1. **后端唯一业务来源**
   - 业务逻辑只能继续收敛到 `services/*`
   - `commands/*` 与 `admin/*handlers.rs` 只能做协议适配

2. **前端唯一业务 UI 来源**
   - 业务标题、文案、布局、样式、结构性组件必须优先落在共享 `features/*` / shared shell / shared tokens
   - Desktop 与 Web 不得长期维护两套业务页面

3. **Web 只允许做壳层，不允许做第二前端系统**
   - Web 可以有登录壳、路由壳、布局壳、鉴权壳
   - Web 不允许复制业务表单、业务列表、业务管理器主视图并长期独立维护

4. **静态 UI 修改必须“一处修改、双端刷新可见”**
   - 如果桌面端修改一个共享标题/文案/布局后，Web 刷新仍需要单独补改，视为 UI 已分叉

5. **单一 Binary 口径不得漂移**
   - Combined / Standalone 只是同一程序的运行模式
   - CLI 不是当前主线
   - GUI 失败自动降级未闭环前，不得写成“完全完成”

6. **阶段治理顺序不得被实际进度反向破坏**
   - 即使 Phase 2 / 3 有局部提前实现
   - 也不能降低 Phase 1 的退出门槛

### 12.3 当前推荐执行顺序（整理后）

1. **先完成 Phase 1 退出门槛**
   - 桌面主路径冒烟
   - Web 最小闭环冒烟
   - 联动一致性冒烟
   - 静态 UI 一致性冒烟
   - 输出前端单源差距清单并冻结

2. **再补 Phase 2 闭环验证**
   - RuntimeMode / ModeSource / detect_runtime_mode() 已有基础
   - 当前重点是 GUI 失败降级、headless 人工验证、模式日志闭环

3. **再执行 Phase 3：前端单源收口**
   - 先消灭桌面页直连 API/Tauri 依赖
   - 再压缩 `src/web-admin/*` 中非壳层业务逻辑
   - 再做静态 UI 一处修改双端刷新专项回归

4. **再进入管理面补漏与体验收口**
   - Pool / Tokens / Logs / Dashboard 若已有能力，先验证是否继续沿 shared service + shared feature 路径成立
   - 若发现分叉实现，优先继续下沉到 service / feature

5. **最后进入工程化、防分叉封口**
   - Tray 制度化回归
   - 双入口一致性回归
   - CI / review 规则
   - 自动化 guard rails

### 12.5 当前开发环节（整理后固定执行）

后续开发统一按以下环节推进，避免出现“代码先走很远，计划、约束、验收全滞后”的情况。

#### 环节 1：先写入计划

每个新增功能或新增收口任务，先补 4 件事：

1. 目标定义
2. 数据流定义
3. 当前状态标记（骨架已接 / 后端未完成 / 联调未完成 / 构建未验证）
4. 分期位置（属于哪一阶段、是否阻塞当前退出门槛）

#### 环节 2：先接共享层，再接壳层

实现顺序固定为：

```text
共享类型 / shared feature / adapter / service
    -> Desktop 壳层入口
    -> Web 壳层入口
```

禁止顺序反过来变成：

```text
先写 Desktop 专用页
-> 再写 Web 专用页
-> 最后再想办法收敛
```

#### 环节 3：半闭环状态必须显式标记

若某功能当前只完成了部分内容，必须明确标记为以下之一：

- 前端骨架已接
- 后端未完成
- 联调未完成
- 构建未验证

禁止把“已接线”写成“已完成”。

#### 环节 4：每轮开发后必须补状态表

每推进一轮功能，都要同步回写：

- 差距是否缩小
- 哪些文件已进入共享层
- 哪些文件仍属于过渡壳层
- 是否通过了构建/类型检查
- 是否完成了人工验证

#### 环节 5：最后才允许写“完成”

只有同时满足以下条件，才允许写“已完成”：

1. 前端骨架完成
2. 后端状态与端点完成
3. Desktop 与 Web 都能实际走通
4. `cargo check`、`pnpm typecheck`、`pnpm build:web-admin`、`pnpm build:renderer` 通过
5. 计划中的 DoD / 冒烟项已回写确认

---

## 13. 现有系统过渡详解（审查补充 / 历史材料）

> 重要说明：本章内容用于保留审查期的补充设计、过渡性实现思路与历史问题分析。
> 
> - 本章**不是**当前主执行基线
> - 若本章任一条目与第 1–11 节主计划正文冲突，以主计划正文为准
> - 本章中的“复制页面”“过渡实现”“旧阶段拆分”等表述，不得覆盖第 5.2A / 5.2B 已定义的纠偏与 guard rails
> - 后续若将本章中的内容吸收为正式方案，必须先回写到主计划正文，再执行

### 13.1 设计层面的问题与优化

#### 1.1 错误响应格式 — 缺失的关键设计

**问题**：当前 `AppError`（`error.rs:24-31`）序列化为**纯字符串**：

```rust
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
```

Tauri IPC 返回纯字符串错误没有问题，但 Web API 需要标准 HTTP 错误响应。WEB_ADMIN_PLAN.md 12.7 只简略提到了统一错误格式，没有给出具体实现方案。

**补充设计**：

需要新增 `AdminError` 枚举，实现 `axum::response::IntoResponse`：

```rust
// admin/error.rs
pub enum AdminError {
    Unauthorized(String),      // 401
    Forbidden(String),         // 403
    NotFound(String),          // 404
    Conflict(String),          // 409
    BadRequest(String),        // 400
    Internal(String),          // 500
}

impl IntoResponse for AdminError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            Self::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            Self::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            Self::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };
        let body = serde_json::json!({ "error": { "message": message, "code": status.as_u16() } });
        (status, Json(body)).into_response()
    }
}

impl From<AppError> for AdminError {
    fn from(e: AppError) -> Self {
        match e {
            AppError::NotFound(msg) => Self::NotFound(msg),
            AppError::Validation(msg) => Self::BadRequest(msg),
            AppError::Database(msg) => Self::Internal(msg),
            AppError::Network(msg) => Self::Internal(msg),
            AppError::Proxy(msg) => Self::Internal(msg),
            AppError::Internal(msg) => Self::Internal(msg),
        }
    }
}
```

**影响**：所有 admin handler 的返回类型应为 `Result<Json<T>, AdminError>`，通过 `From<AppError>` 自动转换现有 DAO 层错误。

---

#### 1.2 refresh_tray_if_enabled 重复代码 — 需要先抽取

**问题**：`refresh_tray_if_enabled` 函数在 3 个文件中完全重复：
- `commands/config.rs:7-16`
- `commands/pool.rs:12-21`
- `commands/channel.rs:10-19`

Web Admin handler 也需要这个功能。如果直接复制第 4 份，维护成本更高。

**优化**：在实施 Web Admin **之前**，先将 `refresh_tray_if_enabled` 抽取为 `lib.rs` 中的 `pub(crate)` 函数：

```rust
// lib.rs
pub(crate) fn refresh_tray_if_enabled(app: &tauri::AppHandle) {
    if EXPERIMENTAL_LAZY_TRAY_REFRESH {
        return;
    }
    if let Ok(new_menu) = build_tray_menu(app) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
    }
}
```

然后 3 个 commands 文件和 admin handler 都调用 `crate::refresh_tray_if_enabled`。

**优先级**：这是 Phase 0 的重构任务，应在 Web Admin 开发前完成。

---

#### 1.3 业务逻辑 service 层 — 设计中完全缺失

**问题**：WEB_ADMIN_PLAN.md 12.4 提到了"Tauri command 和 admin handler 逻辑一致性"，但没有给出具体的 service 层设计。当前所有业务逻辑都直接写在 `#[tauri::command]` 函数中，例如：

- `toggle_entry`（pool.rs:59-71）：toggle + 清除冷却 + 清除失败计数 + tray 刷新
- `update_channel`（channel.rs:123-140）：禁用渠道时自动禁用 entry + tray 刷新
- `create_entry`（pool.rs:96-120）：创建 + add_channel_model_if_missing + tray 刷新
- `start_proxy`（proxy_cmd.rs:17-43）：创建 server + start + 写 config + 刷新 L1

**补充设计**：需要新建 `src-tauri/src/services/` 模块：

```
src-tauri/src/services/
├── mod.rs
├── entry_service.rs    # toggle_entry, create_entry, delete_entry, reorder_entries
├── channel_service.rs  # create_channel, update_channel, delete_channel
├── proxy_service.rs    # start_proxy, stop_proxy
└── config_service.rs   # update_settings + admin lifecycle
```

每个 service 函数签名示例：

```rust
// services/entry_service.rs
pub async fn toggle_entry(
    db: &Database,
    failure_counts: &Arc<RwLock<HashMap<String, u32>>>,
    app_handle: Option<&tauri::AppHandle>,  // None for headless
    id: &str,
    enabled: bool,
) -> Result<(), AppError> {
    db.toggle_entry(id, enabled)?;
    if enabled {
        let _ = db.set_entry_cooldown(id, None);
        if let Ok(mut counts) = failure_counts.try_write() {
            counts.remove(&id);
        }
    }
    if let Some(handle) = app_handle {
        crate::refresh_tray_if_enabled(handle);
    }
    Ok(())
}
```

Tauri command 变为薄包装：

```rust
#[tauri::command]
pub async fn toggle_entry(app: tauri::AppHandle, state: State<'_, AppState>, id: String, enabled: bool) -> Result<(), AppError> {
    services::entry_service::toggle_entry(&state.db, &state.failure_counts, Some(&app), &id, enabled).await
}
```

Admin handler 调用同一个 service：

```rust
async fn toggle_entry(State(state): State<AdminState>, Path(id): Path<String>, Json(body): Json<ToggleBody>) -> Result<Json<()>, AdminError> {
    services::entry_service::toggle_entry(&state.db, &state.failure_counts, state.app_handle.as_ref(), &id, body.enabled).await?;
    Ok(Json(()))
}
```

**影响**：这是 Phase 1 之前的基础工作。如果不做，Phase 1 的 admin handler 会重复大量业务逻辑。

---

#### 1.4 AdminState 与 ProxyState 的状态类型冲突

**问题**：WEB_ADMIN_PLAN.md 提到单端口模式需要 merge 管理路由到代理路由，但没有解决 Axum 的 state 类型冲突。

代理路由使用 `ProxyState`（server.rs:22-30）：

```rust
pub struct ProxyState {
    pub db: Arc<Database>,
    pub settings: Arc<RwLock<AppSettings>>,
    pub circuit_breakers: Arc<RwLock<HashMap<String, CircuitBreaker>>>,
    pub failure_counts: Arc<RwLock<HashMap<String, u32>>>,
    pub app_handle: tauri::AppHandle,
    pub http_client: reqwest::Client,
}
```

Admin 路由需要 `AdminState`（包含 login_sessions 等）。

Axum 的 `.with_state()` 只能绑定一种 state 类型。两种路由合并后，state 类型必须统一。

**解决方案**（文档中未提及）：

方案 A — **Extension 注入**（推荐）：
- 代理路由继续用 `.with_state(proxy_state)`
- 管理路由不使用 `.with_state()`，而是通过 `axum::Extension` 注入 AdminState
- 在 `build_admin_router` 中：`Router::new().route(...).layer(Extension(admin_state))`
- handler 中通过 `Extension(state): Extension<AdminState>` 提取

方案 B — **统一 state 类型**：
- 定义 `CombinedState { proxy: ProxyState, admin: AdminState }`
- 但这样代理路由的 handler 签名都要改，影响太大

**推荐方案 A**，需要在文档中补充。

---

#### 1.5 无头环境检测 — 缺失的具体方案

**问题**：WEB_ADMIN_PLAN.md 3.4 提到"无头环境自动策略"，但没有说明**如何检测**无头环境。

**补充方案**：

```rust
fn is_headless() -> bool {
    // Tauri v2: 如果没有 display，setup 会失败
    // 但对于 Web Admin，我们需要在 setup 之前就知道
    // 方案：检查环境变量或命令行参数
    std::env::var("API_SWITCH_HEADLESS").is_ok()
    || std::env::args().any(|a| a == "--headless")
}
```

或者更简单：不检测无头环境，而是让 Web Admin 在所有环境下都可用（只要配置了用户名密码）。桌面版和 Web 版可以同时运行，互不影响。

---

#### 1.6 静态前端资源嵌入方案 — 缺少具体实现

**问题**：WEB_ADMIN_PLAN.md 12.8 提到了开发/发布两种模式，但没有给出具体的 Rust 实现代码。

**补充实现**：

```rust
// admin/static.rs

#[cfg(debug_assertions)]
pub async fn serve_index() -> impl IntoResponse {
    // 开发模式：从文件系统读取
    match tokio::fs::read_to_string("dist-web-admin/index.html").await {
        Ok(html) => axum::response::Html(html).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(not(debug_assertions))]
pub async fn serve_index() -> impl IntoResponse {
    // 发布模式：从嵌入的字节中读取
    let html = include_str!("../../../dist-web-admin/index.html");
    axum::response::Html(html).into_response()
}
```

**推荐使用 `rust-embed` crate**：

```rust
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "dist-web-admin/"]
struct WebAdminAssets;

pub async fn serve_asset(path: &str) -> impl IntoResponse {
    match WebAdminAssets::get(path) {
        Some(content) => {
            let mime = content.metadata.mimetype();
            ([(header::CONTENT_TYPE, mime)], content.data.to_vec()).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
```

---

### 13.2 后端过渡 — 逐文件改动详解

#### 2.1 `config_dao.rs` — AppSettings 扩展

**当前现状**（`src-tauri/src/database/dao/config_dao.rs:7-23`）：

```rust
pub struct AppSettings {
    pub proxy_enabled: bool,
    pub listen_port: i32,
    pub access_key_required: bool,
    pub circuit_failure_threshold: i32,
    pub proxy_connect_timeout_secs: u64,
    pub circuit_recovery_secs: i64,
    pub circuit_disable_codes: String,
    pub circuit_retry_codes: String,
    pub disable_keywords: String,
    pub locale: String,
    pub theme: String,
    pub autostart: bool,
    pub start_minimized: bool,
    pub show_guide: bool,
    pub default_sort_mode: String,
}
```

**过渡要求**：

在 `default_sort_mode` 字段之后追加 4 个字段：

```rust
pub web_admin_enabled: bool,
pub web_admin_username: String,
pub web_admin_password: String,
pub web_admin_port: i32,
```

**同步改动点**（共 3 处，全在同一文件内）：

| 改动位置 | 改什么 | 注意事项 |
|----------|--------|----------|
| `AppSettings` struct (L7-23) | 追加 4 个字段 | 必须加在 `default_sort_mode` 之后，保持 `serde(default)` 兼容 |
| `Default` impl (L25-45) | 追加默认值 | `web_admin_enabled: false`, `web_admin_username: "".to_string()`, `web_admin_password: "".to_string()`, `web_admin_port: 9099` |
| `get_settings()` (L48-108) | 追加 4 个 `if let Some(v)` 读取块 | 复制现有模式，`web_admin_enabled` 用 `v == "1"`，`web_admin_port` 用 `v.parse().unwrap_or(9099)`，两个 String 字段用 `v.clone()` |
| `update_settings()` (L110-161) | 在 `kv` 数组中追加 4 个元组 | `web_admin_enabled` 用 `if ... { "1" } else { "0" }`，`web_admin_port` 用 `.to_string()`，两个 String 直接引用 |

**注意事项**：
- `web_admin_password` 在 `get_settings()` 中正常读取，但在 Web API 的 `GET /admin/settings` 响应中**必须过滤掉**（见密码掩码章节）
- `serde(default)` 保证老用户升级后反序列化旧 JSON 不会报错（新字段取 Default 值）
- `update_settings()` 当前使用 `INSERT OR REPLACE` 写入所有字段，Web handler 调用时**必须传完整 AppSettings 对象**（不能只传部分字段），否则会覆盖其他字段为默认值

---

#### 2.2 `schema.rs` — 默认配置扩展

**当前现状**（`src-tauri/src/database/schema.rs:135-151`）：

```rust
let defaults = [
    ("proxy_enabled", "1"),
    ("listen_port", "9090"),
    // ... 共 15 个 key
    ("default_sort_mode", "custom"),
];
```

**过渡要求**：

在 `default_sort_mode` 之后追加 4 行：

```rust
("web_admin_enabled", "0"),
("web_admin_username", ""),
("web_admin_password", ""),
("web_admin_port", "9099"),
```

**注意事项**：
- 使用 `INSERT OR IGNORE`（L154），老用户升级时**不会覆盖**已有的值，只补缺失的 key
- 不需要 `ensure_column()` 调用——config 表是 KV 结构，新 key 通过 `INSERT OR IGNORE` 自动补齐
- 不需要旧值迁移——`web_admin_enabled` 默认 `"0"`（关闭），老用户升级后 Web 管理端**不会自动开启**

---

#### 2.3 `lib.rs` — AppState 扩展 + 启动逻辑

**当前现状**（`src-tauri/src/lib.rs:16-21`）：

```rust
pub struct AppState {
    pub db: Arc<Database>,
    pub settings: Arc<tokio::sync::RwLock<AppSettings>>,
    pub proxy: Arc<tokio::sync::RwLock<Option<ProxyServer>>>,
    pub failure_counts: Arc<tokio::sync::RwLock<std::collections::HashMap<String, u32>>>,
}
```

**过渡要求**：

1. **AppState 新增字段**：

```rust
pub admin: Arc<tokio::sync::RwLock<Option<AdminServer>>>,
```

2. **启动逻辑**（在 proxy 自启逻辑之后，约 L67 之后）：

```rust
// Auto-start web admin if enabled
let handle = app.handle().clone();
tauri::async_runtime::block_on(async {
    let app_state = handle.state::<AppState>();
    let settings = app_state.settings.read().await.clone();
    if settings.web_admin_enabled
        && !settings.web_admin_username.is_empty()
        && !settings.web_admin_password.is_empty()
    {
        if settings.web_admin_port == settings.listen_port {
            // 单端口模式：管理路由将合并到 ProxyServer
            let proxy_guard = app_state.proxy.read().await;
            if let Some(proxy) = proxy_guard.as_ref() {
                let admin_router = admin::build_admin_router(&app_state, &handle);
                proxy.merge_admin_router(admin_router).await;
            }
        } else {
            // 双端口模式：独立启动 AdminServer
            let admin_server = AdminServer::new(
                settings.web_admin_port,
                app_state.db.clone(),
                app_state.settings.clone(),
                app_state.proxy.clone(),
                app_state.failure_counts.clone(),
                Some(handle.clone()),
            );
            if let Err(e) = admin_server.start().await {
                log::error!("Failed to auto-start admin server: {e}");
            } else {
                let mut admin_guard = app_state.admin.write().await;
                *admin_guard = Some(admin_server);
                log::info!("Admin server auto-started on port {}", settings.web_admin_port);
            }
        }
    }
});
```

3. **模块声明**（L1-4）：新增 `mod admin;`

4. **invoke_handler 注册**（L128-169）：新增 `commands::admin_cmd::get_admin_status`

**注意事项**：
- **启动顺序关键**：必须先启动 ProxyServer（如果是单端口模式），再合并管理路由
- **无头环境**：`tauri::AppHandle` 在无头环境下可能不存在。AdminState 中的 `app_handle` 字段应为 `Option<tauri::AppHandle>`，admin handler 中对 tray 刷新等操作需要 `if let Some(handle)` 安全处理
- **AppState 初始化**：`admin` 字段初始化为 `Arc::new(tokio::sync::RwLock::new(None))`

---

#### 2.4 `proxy/server.rs` — 支持管理路由合并

**当前现状**（`src-tauri/src/proxy/server.rs:74`）：

```rust
pub async fn start(&self) -> Result<(), String> {
```

**过渡要求**：

改造 `start()` 方法，接受可选的管理路由参数：

```rust
pub async fn start(&self, admin_router: Option<Router>) -> Result<(), String> {
    let mut app = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/v1/chat/completions", post(handlers::handle_chat_completions))
        .route("/v1/models", get(handlers::handle_list_models))
        .layer(cors)
        .with_state(self.state.clone());

    if let Some(admin) = admin_router {
        app = app.merge(admin);
    }
    // ... 后续启动逻辑不变 ...
}
```

**注意事项**：
- **CORS 分层**：代理路由已套 `CorsLayer::new().allow_origin(Any)`，管理路由需要自己的受限 CORS
- **状态类型冲突**：见 1.4 节的 Extension 注入方案
- **现有调用方**：`start()` 签名变更后，`lib.rs:59` 和 `commands/proxy_cmd.rs:28` 都需要改为 `server.start(None)`

---

#### 2.5 `commands/config.rs` — 设置变更副作用处理

**当前现状**（`src-tauri/src/commands/config.rs:115-123`）：

```rust
#[tauri::command]
pub async fn update_settings(app: tauri::AppHandle, state: State<'_, AppState>, settings: AppSettings) -> Result<(), AppError> {
    state.db.update_settings(&settings)?;
    let settings = refresh_settings_l1(&state).await?;
    sync_autostart(&settings);
    refresh_tray_if_enabled(&app);
    Ok(())
}
```

**过渡要求**：

在 `refresh_settings_l1` 之后，增加管理端副作用处理：

```rust
#[tauri::command]
pub async fn update_settings(app: tauri::AppHandle, state: State<'_, AppState>, settings: AppSettings) -> Result<(), AppError> {
    let old_settings = state.settings.read().await.clone();
    state.db.update_settings(&settings)?;
    let new_settings = refresh_settings_l1(&state).await?;
    sync_autostart(&new_settings);
    refresh_tray_if_enabled(&app);
    handle_admin_lifecycle(&state, &old_settings, &new_settings).await;
    Ok(())
}
```

新增 `handle_admin_lifecycle` 函数：

```rust
async fn handle_admin_lifecycle(state: &AppState, old: &AppSettings, new: &AppSettings) {
    let port_changed = old.web_admin_port != new.web_admin_port;
    let enabled_changed = old.web_admin_enabled != new.web_admin_enabled;

    if !port_changed && !enabled_changed {
        return;
    }

    // 关闭管理端
    if old.web_admin_enabled && !new.web_admin_enabled {
        if let Some(admin) = state.admin.write().await.take() {
            let _ = admin.stop().await;
        }
        return;
    }

    // 端口变更或刚开启：重启管理端
    if port_changed || (new.web_admin_enabled && !old.web_admin_enabled) {
        if let Some(admin) = state.admin.write().await.take() {
            let _ = admin.stop().await;
        }
        // start new（复用 lib.rs 中的启动逻辑）
    }

    // 只是密码变了：不需要重启（token 继续有效直到过期）
}
```

**注意事项**：
- **抽取为独立函数**：`handle_admin_lifecycle` 和 `refresh_settings_l1` 应从 `commands/config.rs` 中抽取为 `pub` 函数，供 Tauri command 和 Web admin handler 共用
- **失败回滚**：新端口启动失败时，不应让整个 `update_settings` 失败
- **并发安全**：`state.admin.write().await` 获取写锁，确保 stop 和 start 之间不会被其他操作干扰

---

#### 2.6 新增 `admin/` 模块 — 文件结构与职责

**需要新建的文件**：

```
src-tauri/src/admin/
├── mod.rs          # 模块入口，re-export
├── error.rs        # AdminError 枚举
├── auth.rs         # 登录 + Token 鉴权中间件
├── router.rs       # build_admin_router 函数
├── handlers.rs     # 所有 HTTP handler
├── static.rs       # 静态文件 serve
└── server.rs       # AdminServer 结构（双端口模式）
```

**`admin/auth.rs`**：
- `login_sessions: Arc<RwLock<HashMap<String, Instant>>>` — 内存 Token 存储
- `POST /admin/login` handler — 校验用户名密码，生成 UUID token，存入 login_sessions
- `BearerAuth` middleware — 从 `Authorization: Bearer <token>` 提取 token，查 login_sessions 是否存在且未过期（24h）
- Token 清理：采用惰性策略——每次鉴权时检查过期并移除，不启动定时任务
- 密码校验：第一版用字符串直接比较（`==`），后续升级 hash 时只需改这一处
- `POST /admin/login` 路由必须放在 AuthLayer **之外**

**`admin/router.rs`**：

```rust
pub fn build_admin_router(state: AdminState) -> Router {
    let auth_layer = BearerAuthLayer::new(state.login_sessions.clone());

    let public_routes = Router::new()
        .route("/admin/health", get(handlers::health_check))
        .route("/admin/login", post(auth::login));

    let protected_routes = Router::new()
        .route("/admin/settings", get(handlers::get_settings).put(handlers::update_settings))
        .route("/admin/channels", get(handlers::list_channels).post(handlers::create_channel))
        // ... 其他路由
        .layer(auth_layer);

    let static_routes = Router::new()
        .route("/admin/", get(static::serve_index))
        .route("/admin/assets/*path", get(static::serve_asset));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(static_routes)
}
```

**`admin/handlers.rs`**：
- 每个 handler 解析 HTTP 请求参数 → 调用 service 层 → 返回 JSON
- **必须复用 service 层函数**，不重复实现业务逻辑
- **必须处理联动副作用**（见 service 层设计）

**`admin/server.rs`**：

AdminState 定义：

```rust
#[derive(Clone)]
pub struct AdminState {
    pub db: Arc<Database>,
    pub settings: Arc<RwLock<AppSettings>>,
    pub proxy: Arc<RwLock<Option<ProxyServer>>>,
    pub failure_counts: Arc<RwLock<HashMap<String, u32>>>,
    pub app_handle: Option<tauri::AppHandle>,
    pub login_sessions: Arc<RwLock<HashMap<String, Instant>>>,
}
```

AdminServer 结构参照 ProxyServer：
- `port: i32`、`bind_address: String`（固定 `"127.0.0.1"`）
- `shutdown_tx: Arc<RwLock<Option<oneshot::Sender<()>>>>`
- `start()` / `stop()` / `get_status()` 方法

---

### 13.3 前端过渡 — 逐文件改动详解

#### 3.1 `types.ts` — 新增类型

**当前现状**（`src/types.ts:210-244`）：AppSettings 接口有 15 个字段，无 web_admin 相关字段。

**过渡要求**：

在 `AppSettings` 接口中追加 4 个字段：

```typescript
web_admin_enabled: boolean;
web_admin_username: string;
web_admin_password: string;
web_admin_port: number;
```

在 `DEFAULT_SETTINGS` 中追加默认值：

```typescript
web_admin_enabled: false,
web_admin_username: "",
web_admin_password: "",
web_admin_port: 9099,
```

新增管理端状态类型：

```typescript
export interface AdminStatus {
  running: boolean;
  port: number;
  address: string;
}
```

---

#### 3.2 `api.ts` — 新增管理端 API 函数

**当前现状**（`src/lib/api.ts`）：37 个 invoke 函数，全部通过 Tauri IPC 调用。

**过渡要求**：

新增 1 个函数：

```typescript
export async function getAdminStatus(): Promise<AdminStatus> {
  return invoke("get_admin_status");
}
```

Web Admin 前端（`src-web-admin/api.ts`）需要独立实现所有函数，用 `fetch` 替代 `invoke`。

---

#### 3.3 `SettingsPage.tsx` — 新增 Web 管理设置区块

**当前现状**（`src/pages/SettingsPage.tsx`）：5 个设置区块（Proxy、Security、Circuit、Tray、General），无 Web Admin 相关。

**过渡要求**：

在 "System Tray" 区块之后、"General" 区块之前，新增 "Web 管理" 区块。需要新增 `getAdminStatus` 的 Query，密码字段当后端返回 `"__PROTECTED__"` 时显示空输入框 + placeholder，端口变更时显示单端口模式警告。

---

#### 3.4 `i18n` — 新增翻译 key

需要在 `zh.json` 和 `en.json` 中新增 `settings.webAdmin.*` 系列 key。

---

### 13.4 API 设计补充

#### 4.1 遗漏的 API 端点

WEB_ADMIN_PLAN.md 4.7 的 API 清单遗漏了以下端点：

| 端点 | 方法 | 说明 | 理由 |
|------|------|------|------|
| `/admin/health` | GET | 健康检查（免认证） | 12.6 提到了但 API 清单中没有 |
| `/admin/version` | GET | 获取程序版本 | Web Admin 需要显示版本号 |
| `/admin/channels/:id/limit` | GET | 查询渠道限额 | 复用已实现的 `query_limit` 后端 |
| `/admin/settings` | PATCH | 部分更新设置 | 解决完整对象覆盖问题 |

#### 4.2 API 请求/响应格式规范

文档中没有定义统一的请求/响应格式。补充：

**成功响应**：

```json
{ "data": { ... }, "message": "optional success message" }
```

**列表响应**：

```json
{ "data": [ ... ], "total": 100, "page": 1, "page_size": 20 }
```

**错误响应**：

```json
{ "error": { "message": "Channel not found", "code": 404 } }
```

#### 4.3 长操作端点的特殊性

`fetch_models`（channel.rs:224-276）是一个**长时间操作**（可能 10-30 秒），且内部逻辑非常复杂（endpoint 检测 → 类型校正 → 多协议 fallback）。

`test_entry_latency`（pool.rs:143-222）需要访问 `get_adapter()` 和 `reqwest::Client`，这些在 ProxyState 中，不在 AdminState 中。

**方案**：
- 后端：保持同步请求，设置较长超时（60s）
- 前端：fetch 时设置 `AbortSignal.timeout(60000)`
- 测速逻辑抽取到 service 层，接受 `reqwest::Client` 参数

---

### 13.5 前端架构补充

#### 5.1 Vite 构建配置

WEB_ADMIN_PLAN.md 6.5 只说"新增独立的 vite.config.web.ts"，但没有给出具体内容。

```typescript
// vite.config.web.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  root: 'src-web-admin',
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'),
    },
  },
  build: {
    outDir: '../dist-web-admin',
    emptyOutDir: true,
    rollupOptions: {
      input: 'src-web-admin/index.html',
      output: {
        assetFileNames: 'assets/[name]-[hash][extname]',
        chunkFileNames: 'assets/[name]-[hash].js',
        entryFileNames: 'assets/[name]-[hash].js',
      },
    },
  },
  server: {
    proxy: {
      '/admin': {
        target: 'http://127.0.0.1:9099',
        changeOrigin: true,
      },
    },
  },
});
```

**package.json 新增脚本**：

```json
{
  "scripts": {
    "dev:web": "vite --config vite.config.web.ts",
    "build:web": "vite build --config vite.config.web.ts"
  }
}
```

#### 5.2 Web Admin 前端的 API 层

```typescript
// src-web-admin/api.ts
const BASE_URL = '/admin';

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const token = localStorage.getItem('admin_token');
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
  };

  const response = await fetch(`${BASE_URL}${path}`, { ...options, headers });

  if (response.status === 401) {
    localStorage.removeItem('admin_token');
    window.location.href = '/admin/';
    throw new Error('Unauthorized');
  }

  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: { message: response.statusText } }));
    throw new Error(error.error?.message || 'Request failed');
  }

  return response.json();
}
```

#### 5.3 登录页 + 路由守卫

```tsx
// src-web-admin/App.tsx
function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const token = localStorage.getItem('admin_token');
  if (!token) return <Navigate to="/admin/login" replace />;
  return <>{children}</>;
}

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/admin/login" element={<LoginPage />} />
        <Route path="/admin/*" element={
          <ProtectedRoute>
            <Layout>
              <Routes>
                <Route index element={<DashboardPage />} />
                <Route path="channels" element={<ChannelPage />} />
                <Route path="pool" element={<ApiPoolPage />} />
                <Route path="tokens" element={<TokenPage />} />
                <Route path="logs" element={<LogPage />} />
                <Route path="settings" element={<SettingsPage />} />
              </Routes>
            </Layout>
          </ProtectedRoute>
        } />
      </Routes>
    </BrowserRouter>
  );
}
```

---

### 13.6 安全加固

#### 6.1 登录失败限制

WEB_ADMIN_PLAN.md 7.1 说"登录失败：无限制（第一版）"。但无限制的登录失败是严重的安全风险。

**补充方案**：

```rust
// admin/auth.rs
struct LoginRateLimiter {
    attempts: RwLock<HashMap<String, LoginAttempt>>,
}

impl LoginRateLimiter {
    fn is_blocked(&self, ip: &str) -> bool {
        let attempts = self.attempts.read().unwrap();
        if let Some(attempt) = attempts.get(ip) {
            attempt.count >= 5 && attempt.last_attempt.elapsed() < Duration::from_secs(300)
        } else {
            false
        }
    }
}
```

#### 6.2 CORS 配置细节

```rust
// 代理路由 CORS — 全开
let proxy_cors = CorsLayer::new()
    .allow_origin(Any).allow_methods(Any).allow_headers(Any);

// 管理路由 CORS — 限制
let admin_cors = CorsLayer::new()
    .allow_origin(["http://127.0.0.1:9099".parse().unwrap()])
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH])
    .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);
```

---

### 13.7 关键陷阱与应对

#### 陷阱 1：update_settings 必须传完整对象

**问题**：现有 `update_settings` 使用 `INSERT OR REPLACE` 写入**所有**字段。如果只传部分字段，其余字段会被覆盖为默认值。

**应对**：
- Web Admin 前端的 `PUT /admin/settings` handler 也必须传完整对象
- 或者实现 PATCH 端点做 partial update

#### 陷阱 2：单端口模式下代理 stop 导致管理端断开

**问题**：单端口模式下，管理路由合并到 ProxyServer 的 Router 中。停止代理 → 管理端也停止 → 无法通过 Web 重启。

**应对**：
- Web 端停止代理按钮增加确认弹窗
- 单端口模式下改为"重启代理"（stop + start），避免断开
- 或者隐藏"停止代理"按钮，只显示状态

#### 陷阱 3：AdminState 与 ProxyState 的状态共享

**问题**：Admin handler 需要操作 proxy（start/stop），但 ProxyServer 的 `start()` 方法需要 `tauri::AppHandle`。

**应对**：
- 所有使用 `app_handle` 的地方改为 `if let Some(handle)` 安全处理
- AdminServer 的 `new()` 接受 `Option<tauri::AppHandle>`

#### 陷阱 4：Web handler 与 Tauri command 的逻辑分叉

**问题**：如果 admin handler 和 Tauri command 各自实现业务逻辑，时间久了会产生分叉。

**应对**：
- 抽取 service 层：将"操作 + 联动副作用"封装为独立函数
- Tauri command 和 admin handler 都调用同一个 service 函数

#### 陷阱 5：静态前端资源的开发/发布模式差异

**问题**：开发时需要从文件系统读取，发布时需要嵌入二进制。

**应对**：
- 使用条件编译 `#[cfg(debug_assertions)]` 区分
- 推荐使用 `rust-embed` crate 处理跨平台路径

#### 陷阱 6：密码掩码的连锁问题

**问题**：`GET /admin/settings` 返回掩码，前端 PUT 回传时后端需要识别并跳过。

**应对**：GET 响应中不返回 password 字段（从 JSON 中删除），PUT 请求用 `Option<String>` 接收

#### 陷阱 7：proxy/server.rs 的 start() 签名变更连锁

**问题**：`start()` 从无参改为接受 `Option<Router>`，所有调用方都需要更新。

**当前调用方**：`lib.rs:59`、`commands/proxy_cmd.rs:28` — 改为 `server.start(None)`

---

### 13.8 实施计划优化

#### Phase 0 — 前置重构（新增）

| 任务 | 理由 | 工作量 |
|------|------|--------|
| 抽取 `refresh_tray_if_enabled` 到 lib.rs | 消除 3 处重复，为 admin handler 提供统一入口 | 10 分钟 |
| 抽取 service 层（entry/channel/proxy/config） | 避免 command 和 handler 逻辑分叉 | 1-2 天 |
| 新增 `AdminError` 枚举 | Web API 需要标准 HTTP 错误响应 | 30 分钟 |

#### Phase 1 调整

原计划 Phase 1 是"后端基础（2-3 天）"，建议调整为：

1. **Phase 1a**：配置扩展（config_dao + schema） — 0.5 天
2. **Phase 1b**：admin 模块骨架（auth + router + handlers + server） — 1.5 天
3. **Phase 1c**：lib.rs 集成 + proxy/server.rs 改造 — 0.5 天
4. **Phase 1d**：curl 测试所有接口 — 0.5 天

#### Phase 3 调整

原计划 Phase 3 是"Web Admin 前端（2-4 天）"，建议拆分为：

1. **Phase 3a**：项目脚手架（vite.config.web.ts + api.ts + auth.ts + App.tsx） — 0.5 天
2. **Phase 3b**：LoginPage + Layout + 路由守卫 — 0.5 天
3. **Phase 3c**：各页面复制改造 — 1-2 天
4. **Phase 3d**：联调测试 — 0.5 天

---

### 13.9 验证矩阵

| # | 验证项 | 模式 | 预期 |
|---|--------|------|------|
| 1 | 老用户升级后 config 表补齐 | 任意 | 4 个 web_admin_* key 存在，值为默认值 |
| 2 | 桌面版零影响 | 桌面 | 无任何行为变化 |
| 3 | 设置页 Web 管理区块 | 桌面 | 看到 Web 管理区块，默认关闭 |
| 4 | 开启管理端 | 桌面 | 管理端启动，日志输出启动信息 |
| 5 | 浏览器访问 | Web | 显示登录页 |
| 6 | 登录 | Web | 返回 token，跳转 Dashboard |
| 7 | 错误登录 | Web | 401 JSON 错误响应 |
| 8 | 登录失败限制 | Web | 5 次失败后 300s 内阻止 |
| 9 | 渠道 CRUD | Web | 与桌面版行为一致 |
| 10 | 代理启停 | Web | 代理状态正确变化 |
| 11 | 端口变更 | 桌面 | 旧端口不可用，新端口可用 |
| 12 | 单端口模式 | Web | /admin/ 和 /v1/ 同端口可用 |
| 13 | 单端口 stop 联动 | Web | 停止代理需确认，管理端也停止 |
| 14 | 无头环境 | 无头 | 环境变量配置 + 管理端自动启用 |
| 15 | Tray 联动 | Web | Web 端改排序，桌面 Tray 同步更新 |
| 16 | 设置热生效 | 桌面+Web | Web 端改设置，桌面 L1 缓存已更新 |
| 17 | 老 token 过期 | Web | 改密码后旧 token 继续有效直到 24h 过期 |
| 18 | 错误响应格式 | Web | 所有错误返回 JSON `{ "error": { "message", "code" } }` |
| 19 | 静态资源加载 | Web | /admin/ 加载 index.html，/admin/assets/* 加载 JS/CSS |
| 20 | 密码掩码 | Web | GET /admin/settings 不返回真实密码 |
| 21 | 并发登录 | Web | 同一用户多处登录，各 session 独立 |
| 22 | Token 惰性清理 | Web | 过期 token 在下次请求时被移除 |
| 23 | 长操作超时 | Web | fetch_models 60s 超时正确处理 |
| 24 | Service 层一致性 | 任意 | Web 端和桌面端调用同一 service 函数 |
| 25 | cargo check | 编译 | 无错误 |
| 26 | pnpm typecheck | 编译 | 无错误 |
| 27 | 开发模式热更新 | Web (dev) | 改前端代码后刷新浏览器即可 |
| 28 | 发布模式嵌入 | Web (release) | 单文件分发，不依赖外部文件 |

---

### 13.10 风险评估

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Service 层重构影响桌面版 | 中 | 高 | 重构后运行完整桌面版回归测试 |
| Axum state 类型冲突 | 高 | 中 | 使用 Extension 注入方案，不改代理路由 state |
| 静态资源路径在不同平台不一致 | 低 | 中 | 使用 rust-embed crate 处理跨平台路径 |
| Web Admin 前端两份页面维护分叉 | 中 | 低 | 共享 UI 组件，差异集中在 api.ts |
| 无头环境首次配置门槛 | 中 | 中 | 提供 Docker 示例 + 环境变量文档 |
| 长操作导致浏览器超时 | 中 | 低 | 前端设置合理超时 + 错误提示 |
| 单端口 stop 导致管理端断开 | 高 | 中 | Web 端加确认提示 + 改为重启模式 |
| update_settings 覆盖字段 | 高 | 高 | 完整对象传参 + PATCH 端点 |
