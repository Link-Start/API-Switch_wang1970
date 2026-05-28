# TODO

> 执行级待办清单，按优先级排列。状态标记：⏳ 待开始 / 🔄 进行中 / ✅ 已完成 / ❌ 阻塞

---

## P0

### ⏳ Web Admin 路线落地收尾

- **设置更新与版本冲突闭环**：完善 Web 端设置更新的版本一致性处理
- **登录 / token / 鉴权收尾**：修复 validateToken 网络异常时的逻辑缺陷

### ✅ Web 退出登录有时卡住

**已修复：** 前端退出登录改为乐观更新 + fire-and-forget，不再等待后端响应。同时 Session TTL 从 24 小时缩短为 30 分钟。

**关联文件：**
- `src/App.tsx` — handleLogout() 乐观更新
- `src-tauri/src/admin/handlers.rs` — SESSION_TTL 30 分钟
- `src-tauri/src/admin/auth.rs` — require_auth 续期 30 分钟

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

### ⏳ AUTO 路由下 resolved model 可视化的 OpenAI 流兼容问题

**来源：** 使用 `AUTO` 模型时，用户需要明确知道实际命中的上游模型（resolved model）。当前 API-Switch 在 `/v1/chat/completions` 的 `stream=true` 响应尾部额外追加一条调试文本 chunk，例如：

```text
data: {"choices":[{"delta":{"content":"\n\nmodel: gpt-5.4"}}]}
```

这条输出不是完整合法的 OpenAI `chat.completion.chunk` 对象。像 Zed 这类严格按 OpenAI 兼容流式协议解析的客户端，会在反序列化 `ResponseStreamResult` 时失败，报错：

```text
data did not match any variant of untagged enum ResponseStreamResult
```

**现状定位：**
- `/v1/models` 正常
- 非流式 `/v1/chat/completions` 正常
- 流式前几个标准 chunk 正常
- 问题出在流尾部追加的 resolved model 文本 chunk 不是标准 OpenAI SSE chunk
- 当前前面的标准 chunk 已带有 `model` 字段（如 `"model":"gpt-5.4"`），但部分客户端 UI 不会直接展示，所以现有实现又把 resolved model 作为正文文本附加输出

**关键约束：**
- 不能简单去掉 resolved model 的展示需求，因为 `AUTO` 路由场景下，用户确实需要知道实际在与哪个模型对话
- 但也不能继续输出非标准 chunk，否则会破坏 Zed 等严格客户端兼容性

**解决方向：**
- 方案 A（推荐）：将 resolved model 作为**标准合法的 `chat.completion.chunk`** 输出，而不是当前这种裸 `choices.delta.content` 半结构对象。至少需要补齐 `id / object / created / model / choices[index] / finish_reason` 等标准字段，确保每个 `data:` 都能被 OpenAI 兼容客户端正常解析
- 方案 B：在不污染正文语义的前提下，把 resolved model 通过**标准字段、响应头或自定义 debug 通道**提供给自家 UI；第三方 OpenAI 客户端保持纯标准输出
- 方案 C：评估是否可在最终非流式汇总对象或自定义 metadata 中暴露 resolved model，并由 Web Admin / 自家前端显式展示

**实现注意点：**
- 如果继续把 resolved model 作为 `delta.content` 发出，即使格式合法，也会进入聊天正文，语义上属于“系统附加说明”而非模型原始回答，需要确认这是否符合产品预期
- 如果要兼容第三方客户端（Zed、Cherry Studio、OpenWebUI 某些兼容层等），`stream=true` 时务必只输出客户端能接受的标准 SSE 事件
- 流式结束顺序也要规范，避免在 finish / usage 之后再追加不完整事件

**建议验收：**
- 用 Zed 的 `openai_compatible` provider 直接连接 API-Switch，验证不再出现 `ResponseStreamResult` 解析错误
- 抓取原始 SSE，确认 resolved model 相关事件已是完整合法 chunk
- 确认用户仍能看见 AUTO 实际命中的模型名称

**关联文件：**
- `src-tauri/src/proxy/...` 中 OpenAI Chat Completions 流式输出拼装逻辑
- AUTO 路由 resolved model 注入逻辑
- 前端 resolved model 展示逻辑（若改为 UI 显示）

### ⏳ 对话中显示的模型名来源不明 — model info 注入数据追踪

**来源：** 用户在对话（OpenCode/CLI）中看到的模型标签如 `deepseek-v4-singapore`、`deepseek-v4-flash`、`gpt-5.5` 等，部分名称在当前 AUTO 分组的 api_entries 中并不存在。这些显示名疑似来自：

1. **流式响应尾部注入**（`forwarder.rs` 的 `model_info_delta()`）：路由选中的 `entry.model` 被写为 SSE chunk 注入到正文，客户端将其解析为对话内容显示。
2. **上游透传**：上游返回的响应 `model` 字段（如 OpenAI 原始模型名）未被过滤，直接穿透到客户端。

**问题：**
- `model_info_delta()` 注入的 `"\n\nmodel: {name}"` 被客户端当作正文内容渲染，而不是独立的元数据
- 如果路由选中的模型名与上游响应中的 `model` 字段不同，两者都会被显示，造成混淆
- 部分模型名（`deepseek-v4-singapore`）既不在 api_entries 也不在任何渠道中，可能来自上游响应的 `model` 字段

**关联文件：**
- `src-tauri/src/proxy/forwarder.rs` — `model_info_delta()` 注入逻辑（L1605-L1624）、`stream_chunk_has_model_info_delta()` 检测上游已有模型信息（L1534-L1550）
- `src-tauri/src/proxy/forwarder.rs` — 非流式响应 `resolved_model` 日志字段（L243、L2113）

### ✅ 中间格式到上游协议的字段剥离缺失

**已修复：** 所有主要 adapter 已实现白名单过滤，按"中间协议 → 输出协议时，只有目标协议明确支持的标准字段 + 明确扩展字段 + 语义等价可转换内容才允许输出，其余一律丢弃"的原则。

**实现方式：**
- **OpenAI / Azure**：`build_openai_request_output` / `build_azure_request_output` 显式白名单构建器
- **Claude / Gemini**：`transform_request` 从 `json!({})` 重建对象，只映射已知字段
- **Responses**：`filter_responses_response_fields` 白名单过滤
- **Custom**：历史遗留，仅修改 URL，不做过滤（无需处理）

**关联文件：**
- `src-tauri/src/proxy/protocol/openai.rs` — `OPENAI_REQUEST_ALLOWED_FIELDS` + `OPENAI_EXTENSION_FIELDS`
- `src-tauri/src/proxy/protocol/azure.rs` — `build_azure_request_output`
- `src-tauri/src/proxy/protocol/claude.rs` — `transform_request_to_anthropic` 重建
- `src-tauri/src/proxy/protocol/gemini.rs` — `transform_request_to_gemini` 重建
- `src-tauri/src/proxy/protocol/responses.rs` — `filter_responses_response_fields`

### ✅ Gemini 协议端点兼容

- **端点探测增强**：已实现。`channel_service.rs` 探测时同时尝试 `/v1beta/openai/`（OpenAI 兼容）和 `/v1beta/models`（原生）两种端点，自动校准 base_url。`protocol/gemini.rs` 提供完整的原生端点 URL 构建（`generateContent` / `streamGenerateContent` / `models`）。`join_url` 已处理 `/v1beta` 前缀去重。

---

## P1

### ⏳ 消息角色兼容性

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
## P2

### ⏳ Gemini 原生协议端点补全

- **countTokens / embedContent / batchEmbedContents**：直接转发 Gemini 上游的专用端点

### ⏳ Claude 协议边角一致性优化

- **边角字段严格对齐**：补充 stop_sequence 等官方字段
- **thinking 输出面纯化**：明确官方语义与内部兼容承载的边界
- **流式 SSE 边角事件一致性补测**：补全极端事件组合测试

### ⏳ 渠道启用状态接入路由

**来源：** 当前渠道禁用/启用不影响路由判断，禁用渠道的模型仍可被访问

**方案：** 渠道状态影响路由模型筛选。需要评估影响面——渠道涉及较多关联点，建议引入 L1 缓存

### ⏳ Responses 路径稳定性——拦截 reasoning input 和 Responses 专有字段

**来源：** Responses→Chat 转换后，type:reasoning input 项生成错误消息，include/store/metadata 等 Responses 专有字段泄漏到上游 Chat 请求

**状态：** 已实现基础拦截（input_to_messages 跳过 reasoning 项 + handler 层剥离专有字段）

**后续：** 需要支持思维链正常透传（见 P3）

---

## P3

### ⏳ 思维链支持（Thinking/Reasoning 透传）

**来源：** 当前所有 reasoning/thinking 字段被拦截以保证稳定性。长远需要让上游支持思维链的模型正常接收这些字段

**方案：**
- 改为按 allowlist 而非 denylist 控制
- 仅在已知不支持思维链的 API（如不支持 reasoning_effort 的上游）才拦截
- 需要逐个上游测试确认

### ⏳ Tauri v2 安全基线

- 收紧 CSP 策略、最小化 capabilities

### ⏳ 单实例与窗口状态持久化

- 避免多进程争抢、记住窗口位置

### ⏳ 跨平台 WebView 兼容矩阵

- 建立各平台冒线测试清单

### ⏳ 全局错误语义统一

- 统一 IPC 与 HTTP 错误结构、UI 反馈一致性


## 已完成

- （暂无）
