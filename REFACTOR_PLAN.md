# 代理层架构整改计划（最终交付版）

> **状态**：整改已完成并通过审核。本文档是**完整历史 + 最终架构参考**。
> 测试基线：`cargo test --lib` → **231 passed / 0 failed / 0 ignored**

---

## 一、两条公理（不可违反）

**公理一**：api-switch 是**中转和翻译器**，不是内容修改者。

**公理二**：协议以**各自官方文档**为准，这边进来什么，那边出去必须是一样的（往返无损）。

---

## 二、9 条执行决策（贯彻到最终代码）

1. 中间格式用 OpenAI chat.completions
2. OpenAI 是翻译路径，不是规范仲裁者
3. 10 个翻译器合并成 5 个协议模块
4. 每个协议文件遵循统一范式
5. 每协议顶部 `ENABLE_UNKNOWN_FIELD_PASSTHROUGH` 源码常量，不做 UI 配置
6. 合并基底选择按协议实际情况，以完整度高的一侧为基底
7. Responses 上游方向本轮做完
8. P1（Claude SSE usage=0）顺手修掉
9. 不做：同协议直连、换中间协议、Gemini native、外部框架、流式强类型 IR、UI 配置开关

---

## 三、最终状态总览

### 分支

`fix/claude-responses-proxy-issues`（从 `master` 切出）

### 测试

```
cargo test --lib
→ 231 passed / 0 failed / 0 ignored
```

### 6 个原问题的最终状态（经审核复核）

| 问题 | 状态 | 修复方式 |
|---|---|---|
| P1 Claude 流式 `output_tokens=0` | ✅ 真修 | `ClaudeSSETransformer`：usage 捕获前置 + usage-only 帧补发 `message_delta`（commit `7e450ce`） |
| P2 `stream_options` 无条件覆盖 | ✅ 真修 | `StreamOptionsMiddleware::on_request` 改用 `entry().or_insert()` 合并；`forwarder.rs` 老路径已移除（commit `a7f5897`） |
| P3 UTF-8 切字 → `�` | ✅ 真修 | `proxy::sse::append_utf8_safe` 公共模块 + 5 处调用点接入（commit `0771ec1`） |
| P4 流式 buffer 无上限 | ✅ 真修 | `proxy::sse::MAX_STREAM_BUFFER_BYTES = 10MB` + `stream_buffer_exceeded` 辅助；`forwarder.rs`、`claude.rs`、`responses.rs` 三处流链路全部接入（commit `5df729d`） |
| P5 `model:xxx` 污染 Responses | ✅ 真修 | `should_append_model_info` 加 `caller_kind` 参数，`Responses` 调用方返回 false；删除半成品 `ModelAnnotationMiddleware`（commit `8761455`） |
| P6 第二层流无 idle timeout | ✅ 真修 | `forwarder.rs` 原有 `STREAMING_IDLE_TIMEOUT=300s`；`claude.rs::transform_openai_sse_to_claude_stream` 通过 `tokio::select!` 独立实现一份（commit `5df729d`） |

### 架构违规清单（公理二层面）

| 违规 | 状态 | 位置 |
|---|---|---|
| Claude 请求方向未知字段丢失 | ✅ 已修 | `claude.rs` 顶部 passthrough 块 |
| Claude 响应方向 whitelist 构造 | ✅ 已修 | `openai_to_claude_response` 改为 `clone + edit-in-place` |
| Gemini 请求方向未知字段丢失 | ✅ 已修 | `gemini.rs` 同上 |
| Gemini 响应方向 whitelist 构造 | ✅ 已修 | `openai_to_gemini_response` 同上 |
| Responses 上游 adapter 未实现 | ✅ 已修 | `protocol/responses.rs` 完整实现（commit `3a3caf6`） |
| 10 翻译器分散 | ✅ 已修 | 合并成 5 模块（commit `ec81f69`） |

---

## 四、协议模块统一范式

**一个协议 = 一个文件 = 一套标准套件**：

```rust
// protocol/xxx.rs

// 1. 穿透开关（源码常量，不做 UI 配置）
const ENABLE_UNKNOWN_FIELD_PASSTHROUGH: bool = true;

// 2. ProtocolAdapter trait 实现（上游方向）
pub struct XxxProtocol;
impl ProtocolAdapter for XxxProtocol {
    fn build_chat_url(...) -> String;
    fn apply_auth(...);
    fn build_models_url(...);
    fn parse_models_response(...);
    fn transform_request(...);
    fn transform_response(...);
    fn transform_sse_line(...);
    fn needs_sse_transform(...);
    fn extract_sse_usage(...);
}

// 3. 下游方向翻译（供入口 handler 使用）
pub fn xxx_to_openai_request(...);
pub fn openai_to_xxx_response(...);
pub struct XxxSSETransformer { ... }

// 4. 单元测试
```

**加第 6 个协议的代价**：复制文件、填翻译、`mod.rs` 注册、前端加选项。**核心不改**。

---

## 五、完整 commit 历史

### 阶段 0：测试基线

| Hash | 描述 |
|---|---|
| `bc5c5f7` | Claude 协议 round-trip 测试基线（11 测试） |
| `d3f0300` | Gemini/Azure/OpenAI/Custom round-trip 测试（+8 测试） |
| `3cfd1e8` | Responses 占位测试（+4 ignored） |
| `856ed6b` | 测试兼容 Claude system 字段两种形态 |

**产出**：`protocol/roundtrip_tests.rs`（23 测试，10 绿 + 5 红 + 4 ignored）。

### 阶段 1：流式公共设施

| Hash | 描述 |
|---|---|
| `0771ec1` | 提取 `proxy::sse` 公共模块，修复 P3 |

**产出**：`proxy/sse.rs`（`append_utf8_safe` / `sse_data_payload` + 8 单测）。`handlers.rs` / `forwarder.rs` / `responses_handler.rs` 三处 `from_utf8_lossy` 替换。

### 阶段 2：字段穿透对称化

| Hash | 描述 |
|---|---|
| `884f738` | Claude 双向字段穿透 |
| `3980efa` | Gemini 双向字段穿透 |

**产出**：`claude.rs` / `claude_output.rs` / `gemini.rs` / `gemini_output.rs` 顶部加 `ENABLE_UNKNOWN_FIELD_PASSTHROUGH`。响应方向从 `json!({...})` 改为 `clone + edit-in-place`。

### 阶段 3.1：P1 修复

| Hash | 描述 |
|---|---|
| `de62e99` | P1 初版尝试 |
| `7e450ce` | P1 正式修复 + 清理 541 行死代码 |

**产出**：`ClaudeSSETransformer` usage 捕获前置 + usage-only 帧补发 `message_delta` + `message_delta_emitted` 状态字段。

### 阶段 3.2：Responses 上游 adapter

| Hash | 描述 |
|---|---|
| `6577844` | 前端加 `"responses"` ApiType 选项（Beta） |
| `3a3caf6` | Rust 端 `protocol/responses.rs` 实现 + 解锁 4 占位测试 |

**产出**：795 行的 `ResponsesAdapter`，双向完整翻译，含 `ENABLE_UNKNOWN_FIELD_PASSTHROUGH`。

### 阶段 3.3 + 3.4 + 4.1-4.3：协议合并 + middleware 骨架

| Hash | 描述 |
|---|---|
| `ec81f69` | 合并 10→5 协议模块 + 新增 `middleware.rs` + 提取 Responses helpers |
| `a7f5897` | middleware 接入 forwarder 和 handlers |
| `7d4d221` | 中间件上下文用拥有所有权的 `Arc<RequestContext>`，流式安全 |

**产出**：删除 `claude_output.rs` / `gemini_output.rs` / `azure_output.rs`，全部合并到对应 `protocol/*.rs`。新增 `proxy/middleware.rs`。

### 阶段 4.4-4.6：审核后收口

| Hash | 描述 |
|---|---|
| `5df729d` | P4/P6 补到 Claude SSE + responses_handler 瘦身 |
| `8761455` | **P5 真修**：`should_append_model_info` 加 `caller_kind` 检查；删除 `ModelAnnotationMiddleware` 等 4 个空壳中间件 |
| `9ed002f` | Azure 常量注释说明；删除 `responses_handler` 的冗余本地包装函数 |

**关键转折**：审核发现 `ModelAnnotationMiddleware` 是每 chunk 重复注入、`should_append_model_info` 不看 `caller_kind` 导致 P5 未真修。本轮三 commit 彻底修复。

### 文档

| Hash | 描述 |
|---|---|
| `907ea79`、`b33b3a5`、`75c8ed4`、`2c13abe`、`dfa29e3`、`abc9b0f`、`d455211` | REFACTOR_PLAN.md 迭代 |

---

## 六、最终文件地图

```
src-tauri/src/proxy/
├── mod.rs                 # 模块声明
├── server.rs              # Proxy HTTP server 启动
├── auth.rs                # Access key 校验
├── circuit_breaker.rs     # 熔断器数据结构
├── router.rs              # 模型路由
├── forwarder.rs           # 转发核心（横切逻辑原生实现）
├── middleware.rs          # 中间件 trait + StreamOptionsMiddleware + CallerKind
├── handlers.rs            # 4 个上游入口 handler（chat/messages/gemini/azure）
├── responses_handler.rs   # /v1/responses handler（薄层，翻译逻辑在 protocol/responses.rs）
├── sse.rs                 # SSE 公共设施（UTF-8 安全、buffer 上限、data: 解析）
└── protocol/
    ├── mod.rs             # ProtocolAdapter trait + get_adapter factory
    ├── common.rs          # join_url
    ├── openai.rs          # OpenAI 基准
    ├── custom.rs          # OpenAI 兼容
    ├── claude.rs          # Claude 双向（上游 adapter + 下游翻译器 + SSE 双向）
    ├── gemini.rs          # Gemini 双向
    ├── azure.rs           # Azure 双向（薄）
    ├── responses.rs       # Responses 双向（含 SSE helpers）
    └── roundtrip_tests.rs # 24 个 round-trip 测试
```

---

## 七、架构决策记录

### 为什么保留 `middleware.rs` 只留一个中间件？

审核发现 `ModelAnnotationMiddleware` / `IdleTimeoutMiddleware` / `UsageLoggingMiddleware` / `CircuitBreakerMiddleware` 要么实现有 bug，要么是空壳（注释写"实际逻辑在 forwarder.rs 中"）。真正的决策：

- **`StreamOptionsMiddleware`**：有独立价值（P2 修复逻辑集中在一处），**保留**
- **其余 4 个**：共同特征是"需要跨 chunk 状态"或"需要共享异步资源"——同步 trait 方法无法表达。强行中间件化会增加间接层而无收益。**删除**

注释在 `middleware.rs` 开头详细记录了每个被删中间件的原因，供未来重新考虑时参考。

### 为什么 Azure 有 `ENABLE_UNKNOWN_FIELD_PASSTHROUGH` 但没 if 检查？

Azure OpenAI 的请求/响应与 OpenAI 协议完全兼容，body 字段直通即可。该常量作**范式一致性标记**——所有协议模块顶部都有，便于未来按需扩展时一处切换。azure.rs 内部注释详细说明了这一点。

### 为什么 P5 修复最终没用中间件？

中间件方案（`ModelAnnotationMiddleware::on_sse_chunk`）有三个无法解决的问题：
1. **无去重**：每 chunk 都 push，流式响应会追加 N 次
2. **与老路径重叠**：`forwarder.rs` 的 `model_info_delta` 老路径还在，删不干净（因为老路径有必要的 tool_calls / structured_output 规避逻辑）
3. **切入点不对**：应该在 `[DONE]` 前注入一次，不是每 chunk 注入

正确解法是**在源头决策**：`should_append_model_info` 加 `caller_kind` 参数，Responses 直接返回 false。老路径的去重和规避逻辑保留不动。改动最小，行为最稳。

---

## 八、不做什么（明确拒绝）

- 同协议直连（翻译几毫秒可接受）
- 换中间协议（OpenAI chat.completions 继续）
- 激活 Gemini native dead code
- 引入 LiteLLM / MCP / 其他外部框架
- 流式强类型 IR
- 给用户 UI 配置 passthrough 开关
- 强行把 Idle / Usage / Circuit 逻辑中间件化

---

## 九、遗留可选项（不是 bug，不阻塞 merge）

### Responses 流式翻译简化

`protocol/responses.rs::ResponsesAdapter::needs_sse_transform` 返回 `false`——即 Responses 作为**上游 adapter** 时，SSE 直通不做翻译（v1 简化）。

理由：Responses 作为下游入口（`/v1/responses`）时，SSE 双向翻译已在 `responses_handler.rs` + `protocol/responses.rs::helpers` 里完整实现；但把 Responses **作为上游** 时的 SSE 翻译（chat.completions → Responses 事件）需要额外 300+ 行代码。

当前业务决策：Responses 上游模型极少，本轮 Beta 够用。未来如有 Responses 上游 channel 的实际需求，再补齐 SSE 翻译。

### Azure 自身不激活 `ENABLE_UNKNOWN_FIELD_PASSTHROUGH`

Azure = OpenAI，不需要独立 passthrough 代码。常量保留做一致性标记。如果未来 Azure 协议自身扩展（比如 Azure 专有的 `dataSources`），再激活。

---

## 十、后续开发者须知

### 加一个新协议怎么办

1. 复制 `protocol/openai.rs` 或 `protocol/claude.rs` 做起点
2. 按范式填字段，读该协议的官方文档
3. `protocol/mod.rs` 的 `get_adapter` 加分支
4. 前端 `src/types.ts` 的 `ApiType` 加枚举
5. 前端 `ChannelManager.tsx` 的 `API_TYPES` 加选项
6. 在 `protocol/roundtrip_tests.rs` 加 round-trip 测试
7. 跑 `cargo test --lib` 确保全绿

### 改 `forwarder.rs` 要注意

- **不要**加新的横切中间件除非确信独立价值（参考 StreamOptionsMiddleware 的标准）
- **不要**在 `forward_single` 里做协议特定逻辑（协议逻辑在 `protocol/*.rs`）
- **不要**在 `build_streaming_response` 里做协议翻译（`needs_transform` 分支已足够）
- `should_append_model_info` 如果要加新规则（比如某协议也不能注入），在那个函数里加 `caller_kind` 判断

### 修 bug 前先跑测试

```bash
cd src-tauri && cargo test --lib
```

基线必须是 **231 passed / 0 failed / 0 ignored**。如果基线被破坏，先查 git log 看是哪个 commit 引入，不要直接在新改动上加 patch。

---

## 附录：协议翻译对照

| 概念 | OpenAI chat.completions | Claude | Gemini | Azure | Responses |
|---|---|---|---|---|---|
| 入口路径 | `/v1/chat/completions` | `/v1/messages` | `/v1beta/models/*:generateContent` | `/openai/deployments/*/chat/completions` | `/v1/responses` |
| 消息容器 | `messages[]` | `messages[]` | `contents[]` | `messages[]` | `input[]` |
| system | `{role:"system"}` | 顶层 `system` | 顶层 `systemInstruction` | `{role:"system"}` | 顶层 `instructions` |
| 工具定义 | `tools[].function` | `tools[]` (flat) | `tools[].functionDeclarations[]` | `tools[].function` | `tools[]` (flat) |
| 工具调用 | `choices[].message.tool_calls[]` | `content[].{type:"tool_use"}` | `candidates[].content.parts[].functionCall` | `choices[].message.tool_calls[]` | `output[].{type:"function_call"}` |
| token 用量 | `usage.prompt_tokens/completion_tokens` | `usage.input_tokens/output_tokens` | `usageMetadata.{prompt,candidates}TokenCount` | 同 OpenAI | `usage.input_tokens/output_tokens` |
| 结束原因 | `finish_reason: stop/length/tool_calls` | `stop_reason: end_turn/max_tokens/tool_use` | `finishReason: STOP/MAX_TOKENS` | 同 OpenAI | `status: completed/incomplete/failed` |
| 鉴权 | `Authorization: Bearer` | `x-api-key` + `anthropic-version` | `?key=` query | `api-key` header | `Authorization: Bearer` |
