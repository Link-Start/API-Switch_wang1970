# TODO

> 执行级待办清单，按优先级排列。状态标记：⏳ 待开始 / 🔄 进行中 / ✅ 已完成 / ❌ 阻塞

---

## P0

### ⏳ Web Admin 路线落地收尾

- **设置更新与版本冲突闭环**：完善 Web 端设置更新的版本一致性处理
- **登录 / token / 鉴权收尾**：修复 validateToken 网络异常时的逻辑缺陷

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

### ⏳ 中间格式到上游协议的字段剥离缺失

**来源：** 当前 Responses→OpenAI（中间格式）转换保留全量字段是正确的，但 OpenAI（中间格式）→ 上游协议发送前，缺少按目标协议的字段过滤步骤。Responses 专属字段（如 store、metadata、include、prompt_cache_key、safety_identifier 等）会原样泄漏到非 OpenAI 上游（Claude / Gemini / Azure 等），可能导致上游拒绝请求或行为异常。

**现状：**
- responses_to_openai_chat_request() 末尾通配透传所有未知字段到中间格式 — ✅ 正确，中间格式应该全量携带
- forwarder.rs 的 forward_single() 调用 adapter.transform_request() 后直接发送 — ❌ 缺少目标协议字段剥离
- OpenAiAdapter.transform_request() 只插 model 字段，不清理
- ClaudeAdapter / GeminiAdapter 只做自己协议的格式转换，也不清理 Responses 残留字段
- responses_handler.rs 有事后清理（handler 层剥离 include/store 等），但这只对 Responses handler 生效，其他入口（如 Chat handler 转发到非 OpenAI 上游）不受保护

**冲突记录（后续处理）：**
- 最新裁决标准：**中间协议 -> 输出协议时，只有当前协议明确支持的标准字段 + 明确扩展字段 + 语义等价可转换内容才允许输出，其余一律丢弃**
- 现有 `ResponsesAdapter`、`ClaudeAdapter`、`GeminiAdapter` 等部分旧测试仍保护“未知字段穿透”语义
- 这些旧测试与最新输出边界标准冲突，后续应按新标准调整测试，而不是继续保留默认 unknown passthrough
- 已确认 `Responses` 输出边界也适用该新标准；旧有 request/response passthrough 公理不再作为输出边界规则

**解决方向：**
- 方案 A：每个 adapter 的 transform_request()/输出 response 构造函数内部实现字段白名单 — 只保留目标协议认识的字段，剥掉其他
- 方案 B：在 forwarder.rs 的 forward_single() 里加一个通用的“目标协议字段过滤”步骤，adapter 声明自己协议的字段白名单
- 方案 C：在 ProtocolAdapter trait 上新增显式输出边界过滤接口，并在每个输出协议实现中执行

**关联文件：**
- src-tauri/src/proxy/protocol/responses.rs — responses_to_openai_chat_request() 通配透传
- src-tauri/src/proxy/forwarder.rs — forward_single() 发送前的处理链
- src-tauri/src/proxy/protocol/mod.rs — ProtocolAdapter trait
- src-tauri/src/proxy/protocol/openai.rs / claude.rs / gemini.rs — 各 adapter

### ⏳ Gemini 协议端点兼容

- **端点探测增强**：对不支持原生 Gemini 端点（/v1beta/openai/...）的服务端，需评估是否自动 fallback 或以其他方式尝试识别 Gemini 兼容性

---

## P1

### ⏳ 消息角色兼容性

**来源：** 部分上游（如 M CODIN PLAN 等）拒绝 `messages[]` 中 `role: "system"` 的消息，返回 400 error。

**方案：** 在 forwarder 中将 system role 转为 user role，或合并到第一条 user 消息。按渠道（api_type）差异处理。

**关联：** `src-tauri/src/proxy/forwarder.rs`

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
