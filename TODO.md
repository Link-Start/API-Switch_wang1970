# TODO

> 执行级待办清单，按优先级排列。状态标记：⏳ 待开始 / 🔄 进行中 / ❌ 阻塞

---

## P0

### ⏳ 依赖膨胀分析落地

**来源：** `docs/dependency_analysis.md` 分析显示，在 Termux 上运行 `api-switch` 时，需搬运 ~678 个 `.so`（约 708MB）的 GUI 依赖库（GTK、WebKit2GTK、ICU 等）。即使只用 headless 模式，这些库也会因 ELF NEEDED 被强制载入。

**目标：** 编译 headless-only 版本（剔除 GTK/WebKit 依赖），使 Termux 部署彻底轻量化。

**方案：**
- 编译时引入条件编译（`ENABLE_GUI` 开关），GUI 与 core 解耦
- 分发双版本：`api-switch-server`（headless，~10-20MB）+ `api-switch-desktop`（全量）
- 编译时使用 `-Wl,--as-needed` 等链接选项减少不必要的 NEEDED

**关联文件：**
- `docs/dependency_analysis.md`
- `src-tauri/Cargo.toml` — 依赖管理
- `src-tauri/src/lib.rs` — 入口
- `.github/workflows/release.yml` — 构建分发

### ⏳ 手机浏览器适配 — 当前 UI 框架对移动端支持极差

**来源：** Web Admin 目前使用 Radix UI + Tailwind 的桌面优先布局，在手机浏览器上几乎不可用。参考 GitHub 使用 Primer React（CSS Modules + 原生 HTML 元素）的做法，Radix UI 的 Dialog/Popover/Select 等组件在触屏交互、小屏布局上体验很差，且组件语义化过重难以按需定制。

**现有问题：**
- 固定宽度侧边栏（`w-56`）在手机上占满整个屏幕
- 表格/列表（PoolManager、ChannelManager）在手机上无法正常浏览
- Radix UI Dialog 在手机上全屏覆盖体验差
- 没有移动端底部导航栏替代侧边栏
- Select/DropdownMenu 等下拉组件在触屏上交互违和
- 可拖拽排序（`@dnd-kit`）在触屏上基本不可用

**建议方案：**
- 参考 GitHub Primer React 的轻量风格，替换或跳过部分 Radix UI 重型组件
- 移动端用底部导航栏（Bottom Tab Bar）替代固定侧边栏
- 关键页面（渠道管理、API 管理、令牌）提供简化的移动端视图
- 使用 Tailwind 响应式断点（`sm:`/`md:`/`lg:`）重构布局
- 表格/列表改为卡片式布局，适配小屏
- 拖拽排序提供"长按拖动手柄"模式，或回退到列表排序按钮

**关联文件：**
- `src/features/shell/MainShell.tsx` — 侧边栏 + 主内容布局
- `src/features/pool/PoolManager.tsx` — 拖拽排序 + 复杂列表
- `src/features/channels/ChannelManager.tsx` — 表格列表
- `package.json` — Radix UI 组件库依赖
- `src/components/ui/` — 基础 UI 组件

### ⏳ 对话中显示的模型名来源不明 — model info 注入数据追踪

**来源：** 用户在对话（OpenCode/CLI）中看到的模型标签如 `deepseek-v4-singapore`、`deepseek-v4-flash`、`gpt-5.5` 等，部分名称在当前 AUTO 分组的 api_entries 中并不存在。

**状态（2026-05-30）：** ✅ 主因已查明并修复（见文末「✅ 已完成」区「模型名显示统一」）。结论：唯一会让用户看到模型名的是 AS 的 `model_info_delta` 注入（显示的就是命中的 `entry.model`，正确）；上游响应顶层 `model` 字段是 JSON 元数据，客户端不渲染。原"来源不明"是因为 Claude 直通路径此前不注入，AUTO 命中不同协议时显示时有时无。

**剩余可选项：**
- 上游响应顶层 `model` 字段是否需要改写成 `entry.model`（目前透传上游真实名，仅元数据、不显示，影响小）。
- 非流式响应路径的注入与流式口径核对（流式已统一）。

---

## P1

### ⏳ 消息角色兼容性

> **关联：** mid-conversation system 故障（已完成，见文末）走的是「同协议直通」绕开了 messages 角色翻译；本条针对的是**跨协议**转换时 messages 内部 role 的归一化，仍待做。

**来源：** 部分上游拒绝 `messages[]` 中非常规 role 的消息，返回 400 error。

**已观测到的错误：**
1. `role: "system"` 被部分上游（如 M CODIN PLAN 等）拒绝
2. `role: "developer"`（OpenAI Responses API 的 role）被 OpenInference 等不支持的上游拒绝：
   ```
   messages[0].role: unknown variant `developer`,
   expected one of `system`, `user`, `assistant`, `tool`
   ```
   注：顶层白名单已拦截非常规字段，但 `messages` 数组内部每个消息的 `role` 字段未做过滤。

**方案：** 在 forwarder 或 protocol adapter 的 `transform_request` 中，对 messages 内部 role 做归一化（`developer` → `system`，部分上游 `system` → `user`）。按渠道（api_type）差异处理。

**关联：** `src-tauri/src/proxy/forwarder.rs`、`src-tauri/src/proxy/protocol/*.rs` 各 adapter 的 `transform_request`

---

### ⏳ Responses→Chat 转换层两个参数缺陷（2026-05-30 分析）

**来源：** DB 错误日志分析，CODEX 来源今日 48 个错误中大部分由这两个缺陷引起，扇出到所有上游渠道产生倍增效应。

**缺陷 1：	ools: [] 空数组未被清理**

- **表现：** litellm.BadRequestError: [] is too short - 'tools'
- **根因：** convert_tools([]) 正确返回 None（跳过空数组），但 esponses_to_openai_chat_request() L543-548 的「透传未知字段」循环把原始请求的 	ools: [] 又写回 chat_body，覆盖了 convert_tools 的判断。
- **修复：** passthrough 循环应跳过已被显式处理的字段（	ools、esponse_format、messages 等），或在循环结束后再次删除空 tools。

**缺陷 2：esponse_format 缺少 json_schema 子字段**

- **表现：** 'response_format.json_schema' is required when 'response_format.type' is 'json_schema'
- **根因：** esponses_to_openai_chat_request() L506-510 做 	ext.format → response_format 映射时原样 clone，不校验 json_schema 是否存在。Codex 发了 {type: "json_schema"} 但没带 schema 内容，直接透传到上游。
- **修复：** 映射后校验：	ype == "json_schema" 时若无 json_schema 字段，删除整个 esponse_format 或降级为 {type: "text"}。

**影响渠道：** aigc-llm.mgtv.com、nvidia、魔塔社区、linlong.xyz、sensenova 等全线报错。

**关联文件：**
- src-tauri/src/proxy/protocol/responses.rs — esponses_to_openai_chat_request()、convert_tools()

---

### ⏳ Responses hosted tools 降级链路的 high risk 拒绝定位（2026-06-01）

**来源：** 已确认必须区分两类注入：

- `Responses -> Chat/non-Responses` 时，为 hosted tools 缺失做能力降级提示注入，避免下游继续假定 server-side tool 可用。
- `forwarder` 里的 `model: xxx` 仅是模型名展示注入，和 server call / hosted tools 无关；本次已恢复 `CallerKind::Responses` 禁注入，避免污染 `response.output_text`。

**当前结论：**

- `Responses -> Responses` 同协议透传正常，不需要这条降级提示。
- `OpenAI Chat -> Responses` 也不会触发 hosted-tools 降级注入。
- 仍未彻底定位的是：`Responses -> Chat/non-Responses` fallback 失败时，究竟是哪个字段触发上游 high risk / 拒绝，需要失败现场原始协议数据来确认。

**复现与排查要求：**

- 仅在复现期临时开启设置项 `record_raw_protocol_data`。
- 该设置默认关闭，且只在失败日志写入 `usage_logs.other.raw_protocol`，成功请求不记录。
- 复现后重点比对：`caller_kind`、`channel_api_type`、`gateway_body`、`upstream_body`、`error_body`、`upstream_response_body`、`x_request_id`。
- 对照同一请求在 `Responses -> Responses` 透传链路与 `Responses -> Chat` 降级链路的差异，确认触发 high risk 的具体字段或提示文本。

**关联文件：**

- `src-tauri/src/proxy/protocol/responses.rs`
- `src-tauri/src/proxy/forwarder.rs`
- `src/pages/LogPage.tsx`
- `src/features/settings/SettingsEditor.tsx`

---

## P2

### ⏳ 清理排序模式（fastest/latest）死代码

**来源：** `default_sort_mode`(fastest/latest/custom)在**实际路由 `resolve` 里从不生效**——`resolve` 的 `_sort_mode` 参数被忽略，`available_entries` 内部写死 `sort_by_index`。`apply_sort_mode` 仅在 `load_sorted_entries`(`/v1/models` 模型列表展示)被调用。即：排序模式只影响**列表展示顺序**，不影响 **AUTO 实际转发**（后者恒按 `sort_index`）。

**结论：** 非紧急、无实际故障。清理牵涉前后端 + config + DB（影响大），暂缓，单独处理。

**关联（清理范围）：**
- 后端：`src-tauri/src/proxy/router.rs`(`apply_sort_mode`/`sort_by_latency`/`sort_by_release_date`/`parse_response_ms`/`parse_release_date`、`resolve` 的 `_sort_mode`)、`handlers.rs`/`responses_handler.rs` 各处 `sort_mode` 传参
- config：`config_dao.rs`/`schema.rs` 的 `default_sort_mode`（DB 留废弃列）
- 前端：`src/types.ts`(`ModelSortMode`)、`src/lib/modelsCatalog.ts`、`i18n` 的 `latest`/`fastest`

### ⏳ Gemini 原生协议端点补全

- **countTokens / embedContent / batchEmbedContents**：直接转发 Gemini 上游的专用端点

### ⏳ 渠道启用状态接入路由

**来源：** 当前渠道禁用/启用不影响路由判断，禁用渠道的模型仍可被访问

**方案：** 渠道状态影响路由模型筛选。需要评估影响面——渠道涉及较多关联点，建议引入 L1 缓存

---

## P3

### ⏳ 全局错误语义统一

- 统一 IPC 与 HTTP 错误结构、UI 反馈一致性

---

## ✅ 已完成（2026-05-30，本地 master 未推送）

> 本次会话围绕 Claude Opus 4.8 / Claude Code v2.1.154 的协议变化做的一系列修复，均已 `cargo test` 通过 + 真实 Claude CLI 经代理实战验证。详见 `docs/protocol-passthrough-fix-plan.md`。

### Claude 同协议直通 + CLI 身份头透传
mid-conversation system（`role:"system"` 置于 messages 中间）导致 Claude→Claude 中转失效。解法：上游同为 claude/anthropic 时**字节级原样直通**原始请求体（仿 Responses 的 `__as_raw_responses_req`），由上游自行处理 mid-system；并透传真实 Claude CLI 的身份/能力头（`anthropic-beta` 含 `mid-conversation-system-2026-04-07`、`user-agent`、`x-app`、`x-stainless-*`）。设计天然抗 CLI 升级（原样转发，无需跟改）。
- commits: `809e040`(直通) `34b21c1`(头透传) `455e8db`(直通流模型名)

### 入口全穿透 / 出口黑名单过滤（分层规则落地）
确立规则：**入口（A→OpenAI 中间）不过滤、全穿透；出口（中间→目标 B）才按「标准/扩展/语义对应」三类保留、其余抛弃，且不为老旧形态做向后兼容**。修复 Claude CLI→OpenAI 类上游的 `400 UnsupportedParamsError`（`thinking`/`context_management` 等 Anthropic 专有字段泄漏）。OpenAI/Gemini/Azure 出口由白名单改黑名单（保留未知/未来字段，只丢已知外来字段）。
- commits: `cd99509` `fe553b3` `e51b76e` `5b63c70` `1d861ba`

### 模型名显示统一（AUTO 知道在跟谁对话）
此前只有 OpenAI/Gemini/Azure 渠道注入 `model: xxx`，Claude 直通不注入 → AUTO 命中不同协议时显示时有时无。补：Claude 直通流用 Anthropic 原生 `content_block`（start/text_delta/stop）在 `message_stop` 前注入实际命中模型名；复用既有防刷屏闸（每流一次、有 tool_use 则跳过）；受全局 `show_conversation_model` 开关控制。
- commit: `455e8db`

### IN TOKEN 0 修复（缓存 token 计入 prompt）
实测发现 nzbrr 回传 `input_tokens:0` 但真实 prompt 在 `cache_read_input_tokens`（如 429758）+ `cache_creation_input_tokens`。新增 `anthropic_prompt_tokens()` = input + cache_read + cache_creation，流式（message_start/message_delta）与非流式路径统一。实战验证：prompt 从 0 → 439881。
- commits: `e86d672` `3e4222e`

### CI release 重试
`softprops/action-gh-release` 偶发 `other side closed`，加一次重试容错。
- commit: `cd13fc4`

---

## 📋 策划文档摘要

### docs/GENERIC_AGENT_FLOATING_AGENT_PLAN.md — Agent 集成方案

**目标**：右下角独立机器人入口，点击后自动连接 GenericAgent 并打开独立对话窗口

**当前状态**：方案设计阶段

**核心设计**：
- 独立透明小窗口 `agent-launcher`（always_on_top）
- 独立对话窗口 `agent-chat`（不依赖主窗口）
- 完整 pipeline：检查配置 → 启动 proxy → 启动 GA adapter → 等待 ready → 打开窗口

**待办**：
- 实现 `agent-launcher` 窗口
- 实现 `ensure_runtime_ready` 流程
- 实现 `agent-chat` 窗口及连接逻辑

**优先级建议**：P2（核心功能稳定后）

---

### docs/security-audit.md — 公网安全审核

**审核日期**：2026-05-15（v0.6.12）

**风险统计**：6 个高风险、4 个中风险、3 个低风险

**高风险问题**：
1. **RISK-01**: CORS 完全开放（允许任意源）
2. **RISK-02**: 默认弱密码 admin/admin，首次运行不强制修改
3. **RISK-03**: 数据库明文存储 API Key
4. **RISK-04**: 调试模式跳过所有认证
5. **RISK-05**: 无请求体大小限制（proxy 层 32MB）
6. **RISK-06**: 缺少速率限制和 IP 黑名单

**优先级建议**：
- 本地使用：P3（当前架构面向本地/内网）
- 公网部署前：P0（必须修复 RISK-01~06）
