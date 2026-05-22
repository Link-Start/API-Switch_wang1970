# TODO

> 执行级待办清单，按优先级排列。状态标记：⏳ 待开始 / 🔄 进行中 / ✅ 已完成 / ❌ 阻塞

---

## P0

### ⏳ Web Admin 路线落地收尾

- **设置更新与版本冲突闭环**：完善 Web 端设置更新的版本一致性处理
- **登录 / token / 鉴权收尾**：修复 validateToken 网络异常时的逻辑缺陷

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
