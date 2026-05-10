# 代理层架构整改计划（完备版）

> **下次会话接手必读**：本文档既是路线图也是状态快照。按"第三节：当前状态"对齐进度，
> 按"第六节：剩余工作清单"往下执行。所有决策已锁死，不要再重新讨论原则。

---

## 一、两条公理（不可违反）

**公理一**：api-switch 是**中转和翻译器**，不是内容修改者。

**公理二**：协议以**各自官方文档**为准，这边进来什么，那边出去必须是一样的（往返无损）。

---

## 二、9 条执行决策（已对齐，不再讨论）

1. 中间格式用 OpenAI chat.completions，不换
2. OpenAI 是翻译路径，不是规范仲裁者；每个协议以自己官方文档为准
3. 当前 10 个翻译器合并成 5 个协议模块
4. 每个协议文件遵循统一范式（见第四节）
5. 每协议顶部一个 `ENABLE_UNKNOWN_FIELD_PASSTHROUGH` 源码常量，不做 UI 配置
6. 合并基底选择：
   - `OpenAI`：保持现状（基准协议，无需翻译）
   - `Claude`：以上游 `claude.rs` 为基底（更完整），下游方向补齐
   - `Gemini`：以下游 `gemini_output.rs` 为基底，保持 OpenAI 兼容端点方案
   - `Azure`：清理死码，合并成薄文件
   - `Responses`：以下游 `responses_handler.rs` 翻译部分为基底，**本轮补齐上游方向**，Beta 标记
   - `Custom`：保持现状
7. Responses 上游方向在阶段 3 一次做完
8. P1（Claude SSE usage=0）在阶段 3 合并重写时修掉
9. 不做：同协议直连、换中间协议、Gemini native、引入外部框架、流式强类型 IR（推迟到未来独立项目）

---

## 三、当前状态（2026-05-10，9 个 commit 后）

### 分支

`fix/claude-responses-proxy-issues`，从 `master` 切出，已提交 10 个 commit。

### Commit 序列（从旧到新）

| # | Hash | 阶段 | 描述 |
|---|---|---|---|
| 1 | `bc5c5f7` | 0 | 加入 Claude 协议 round-trip 测试基线（14 绿 / 3 红） |
| 2 | `d3f0300` | 0 | 加入 Gemini/Azure/OpenAI/Custom round-trip 测试（共 19 测试，14 绿 / 5 红） |
| 3 | `3cfd1e8` | 0 | 加入 Responses 占位测试（4 ignored） |
| 4 | `884f738` | 2 | Claude 翻译字段穿透（双向） |
| 5 | `3980efa` | 2 | Gemini 翻译字段穿透（双向） |
| 6 | `0771ec1` | 1 | 提取 `proxy::sse` 公共模块，修复 P3 |
| 7 | `856ed6b` | 0.fix | 测试兼容 Claude system 字段两种形态 |
| 8 | `de62e99` | 3.1a | P1 初版修复 |
| 9 | `7e450ce` | 3.1b | P1 完成修复 + 清理死代码 -541/+3 |
| 10 | `907ea79` | docs | REFACTOR_PLAN.md 详化 |

### 测试状态

```
cargo test --lib
→ 215 通过 / 0 失败 / 4 ignored
```

4 个 ignored 在 `src-tauri/src/proxy/protocol/roundtrip_tests.rs` 的 `responses_roundtrip` 模块：
- `adapter_registered_for_responses`（第 685 行）
- `request_openai_to_responses_upstream`（第 699 行）
- `response_responses_to_openai_upstream`（第 726 行）
- `upstream_unknown_field_passthrough`（第 755 行）

实现完成后删除 `#[ignore]` 属性即可启用。

### Working tree 未提交改动（**重要**）

```
 M src-tauri/src/proxy/handlers.rs          （Phase 3.2 初稿，+11/-？）
 M src-tauri/src/proxy/server.rs            （Phase 3.2 初稿，+2/-？）
 M src/features/channels/ChannelManager.tsx （加 Responses Beta 选项）
 M src/types.ts                             （加 ApiType "responses"）
?? src-tauri/src/proxy/protocol/responses.rs（Phase 3.2 核心文件，795 行）
```

**这些是阶段 3.2 的半成品改动**，下次会话接手时要：
1. 检查 `responses.rs` 内容完整性
2. 在 `protocol/mod.rs` 注册 adapter
3. 删除 4 个测试的 `#[ignore]`
4. 跑测试、调整实现直到全绿
5. 提交

### 6 个原问题状态

| 问题 | 状态 | 修复方式 | commit |
|---|---|---|---|
| P1 Claude 流式 output_tokens=0 | ✅ 已修 | ClaudeSSETransformer 把 usage 捕获前置；遇到 usage-only 帧补发 message_delta | `7e450ce` |
| P2 stream_options 无条件覆盖 | ⏳ 待修 | 阶段 4：改 insert 为 entry().or_insert() 合并 | — |
| P3 UTF-8 切字 → � | ✅ 已修 | 抽出 `proxy::sse::append_utf8_safe`，3 处 `from_utf8_lossy` 改用 | `0771ec1` |
| P4 流式 buffer 无上限 | 🟡 部分 | `responses_handler` 原生有 10MB 上限，其他待阶段 3.3 合并时统一 | — |
| P5 model:xxx 污染 Responses | ⏳ 待修 | 阶段 4：`model_info` 注入中间件化，Responses 入口不装配 | — |
| P6 第二层流无 idle timeout | ⏳ 待修 | 阶段 4：`IdleTimeoutMiddleware` | — |

### 已解决的架构违规（公理二层面）

- ✅ Claude 请求方向未知字段穿透（`884f738`，`claude.rs` L132-141）
- ✅ Claude 响应方向从 whitelist 构造改为 clone+edit-in-place（`884f738`）
- ✅ Gemini 请求方向未知字段穿透（`3980efa`）
- ✅ Gemini 响应方向从 whitelist 构造改为 clone+edit-in-place（`3980efa`）

### 尚未解决的架构违规（公理二层面）

- ⏳ Azure 响应方向：`.clone()` 意外合格（因为 Azure = OpenAI），但未加 `ENABLE_UNKNOWN_FIELD_PASSTHROUGH` 常量统一
- ⏳ OpenAI / Custom：基准协议不翻译，不需要处理
- ⏳ Responses 双向：`responses.rs` 已写入但未注册，阶段 3.2 收尾

---

## 四、协议模块统一范式

**一个协议 = 一个文件 = 一套标准套件**。所有 5 个现有协议和未来新增协议都这样。

```rust
// protocol/xxx.rs

// 1. 穿透开关（源码常量，不做 UI 配置）
/// 是否在翻译时穿透本协议官方文档未定义的字段。
/// 默认 true：贯彻"中转不丢失"公理。
/// 应急 false：仅传官方已知字段。如果某上游开始对未知字段返回 400，
/// 定位到是穿透行为导致后改此常量为 false，发布新版本。
const ENABLE_UNKNOWN_FIELD_PASSTHROUGH: bool = true;

// 2. 实现 ProtocolAdapter trait
pub struct XxxProtocol;
impl ProtocolAdapter for XxxProtocol {
    // URL 构造 / 鉴权 / 模型列表
    fn build_chat_url(...) -> String;
    fn apply_auth(...);
    fn build_models_url(...);
    fn parse_models_response(...);

    // 上游方向：OpenAI 中间格式 → 本协议
    fn transform_request(&self, body: &mut Value, model: &str);
    fn transform_response(&self, body: &mut Value);
    fn transform_sse_line(&self, line: &str) -> Option<String>;
    fn needs_sse_transform(&self) -> bool;
    fn extract_sse_usage(&self, line: &str) -> (i64, i64);
}

// 3. 下游方向翻译（供入口 handler 使用）
pub fn xxx_to_openai_request(xxx: &Value) -> Value;
pub fn openai_to_xxx_response(openai: &Value) -> Value;
pub struct XxxSSETransformer { ... }

// 4. 单元测试
#[cfg(test)]
mod tests {
    // round-trip 测试
    // 未知字段穿透测试
    // 官方文档样本测试
}
```

**加第 6 个协议的代价**：复制已有文件、填翻译逻辑、在 `mod.rs` 的 `get_adapter` 加一行、前端 `ApiType` 加选项。**核心不动**。

---

## 五、已完成部分的技术细节（供回滚 / 审查参考）

### 阶段 0：测试基线（commits 1-3、7）

**文件**：
- `src-tauri/src/proxy/protocol/roundtrip_tests.rs`（新建，总 779 行，含 23 个测试）
- `src-tauri/src/proxy/protocol/mod.rs`（修改：加 `#[cfg(test)] mod roundtrip_tests;`）
- `REFACTOR_PLAN.md`（新建）

**测试组织**：
```
mod claude_roundtrip       11 tests（8 绿基线 + 3 红目标）
mod gemini_roundtrip        4 tests（2 绿 + 2 红）
mod azure_roundtrip         2 tests（全绿）
mod openai_roundtrip        2 tests（全绿）
mod responses_roundtrip     4 tests（全 ignored，待阶段 3 启用）
mod helpers                 1 test（辅助函数自测）
```

**初始红测试**：
- `request_unknown_field_passthrough_top_level`（阶段 2 修）
- `response_unknown_field_passthrough`（阶段 2 修）
- `sse_claude_usage_tokens_not_dropped`（阶段 3 修）
- `request_unknown_field_passthrough`（Gemini，阶段 2 修）
- `response_unknown_field_passthrough`（Gemini，阶段 2 修）

**关键测试点**：
- **round-trip**：`A → openai → A'`，要求 `A ≡ A'`
- **穿透**：输入带 `x_api_switch_tracking_id` 之类官方未定义字段，输出必须还在
- **usage**：Claude 流式帧序列模拟 OpenAI 真实帧（role / content / finish / usage-only / [DONE]），断言 `message_delta.usage.output_tokens` 能拿到真实值

### 阶段 1：提取 `proxy::sse` 公共模块（commit `0771ec1`）

**新文件**：`src-tauri/src/proxy/sse.rs`（175 行）

导出两个函数：
- `append_utf8_safe(buffer, remainder, bytes)`：跨 chunk UTF-8 安全拼接
- `sse_data_payload(line)`：解析 SSE `data:` 行

**原理**：维护一个 `utf8_remainder: Vec<u8>` 保存"上次留下的不完整 UTF-8 字节"，新 chunk 来了先拼到尾巴，然后按 `std::str::from_utf8` 的 `valid_up_to()` 切分——合法前缀推入 buffer，不完整的字节留存继续等下一轮。

**修改点**：
- `handlers.rs` 的 `handle_messages` 流式 `unfold` 循环：`sse_buffer.push_str(&String::from_utf8_lossy(&chunk))` → `super::sse::append_utf8_safe(&mut sse_buffer, &mut sse_utf8_remainder, &chunk)`；`unfold` 的 state tuple 从 3 元改 4 元，3 处 return 点同步
- `forwarder.rs` 的 `transform_sse_chunk` 和 `append_and_parse_sse`：签名加 `remainder: &mut Vec<u8>` 参数，内部改用公共函数；所有 14 处调用方（1 处生产 + 13 处测试）同步加 `let mut remainder = Vec::new()`
- `responses_handler.rs`：原有的 `append_utf8_safe` / `sse_data_payload` 私有函数改为调用 `super::sse::` 的同名公共函数

**副作用修复**：P3（UTF-8 切字）全路径消除。

**新增测试**：8 个 `proxy::sse::tests::*`（ASCII / 中文 / emoji / 极端单字节流 / data: 解析）。

### 阶段 2：字段穿透对称化（commits `884f738`、`3980efa`）

**Claude 方向**：

文件 `protocol/claude.rs`：
- 顶部新增 `const ENABLE_UNKNOWN_FIELD_PASSTHROUGH: bool = true;`
- `transform_request_to_anthropic` 最后新增 passthrough 块：遍历原 body 剩余字段 insert 到 anthropic 结果里
- `transform_response_from_anthropic` 改造：以前是 `json!({"id": ..., "object": "chat.completion", ...})` 构造新对象，现在改为 `let mut response_body = obj.clone(); 在 clone 上 edit-in-place`
- 加了 `if !ENABLE_UNKNOWN_FIELD_PASSTHROUGH` 分支，关闭时只保留 OpenAI 官方白名单字段

文件 `protocol/claude_output.rs`（下游方向）：
- 顶部新增常量
- `claude_to_openai_request` 新增 passthrough 块
- `openai_to_claude_response` 改造：从 `json!({"id": claude_id, "type": "message", ...})` 改为 `let mut result = openai.clone(); 在 clone 上 edit-in-place`
- 加了关闭穿透时的白名单分支

文件 `forwarder.rs`：forwarder 里的 `build_streaming_response` 等函数做了配合调整，使流式上游响应的未知字段也能经过 transform 保留。

**Gemini 方向**（`3980efa`）：

文件 `protocol/gemini.rs`：顶部加常量（标 `#[allow(dead_code)]` 因为 Gemini 走 OpenAI 兼容端点，此常量留给未来激活 native 方案用）。

文件 `protocol/gemini_output.rs`：
- 顶部加常量
- `gemini_to_openai_request` 新增 passthrough 块
- `openai_to_gemini_response` 从 `json!({...})` 构造改为 `.clone()` + edit，加关闭穿透的白名单分支

**效果**：5 个原来失败的字段穿透测试全部变绿。

### 阶段 3.1：P1 修复（commits `de62e99` + `7e450ce`）

**文件**：`protocol/claude_output.rs`

**根因**：`ClaudeSSETransformer::transform_chunk` 原来的流程：
```
1. parse chunk
2. let Some(choice) = chunk.choices.get(0) else { return events; };  ← 空 choices 直接退
3. capture usage from chunk  ← 永远到不了，因为帧 N+2 的 choices=[]
```

OpenAI 官方帧序列：`role / content* / {finish_reason} / {choices=[], usage} / [DONE]`。第 4 帧 `choices=[]`，第 3 帧已经触发 `message_delta` emit 且用的是 `self.usage_output_tokens = 0`。

**修复**：
1. **调序**：usage 捕获从 `let Some(choice)` 之后挪到之前
2. **补发**：在 early return 分支内判断如果 `message_delta` 已 emit 且 usage 已更新，补发一次 `message_delta`（Claude 协议允许多次 message_delta）
3. **记号**：新增 `message_delta_emitted: bool` 字段跟踪状态
4. **清理**：`protocol/mod.rs` 的 `pub use` 清理阶段 2 已删除的符号（`openai_to_azure_response` / `transform_azure_error` / `AzureSSETransformer` / `transform_gemini_error` / `GeminiSSETransformer`）—— 这些原来是死代码，在阶段 2 的 Gemini/Azure 重构里删掉了但 `pub use` 残留导致编译失败

**相关死代码清理**：`7e450ce` 一次性删除 541 行（`azure_output.rs` 大半 + `gemini_output.rs` 里 `build_gemini_native_*` / `transform_request_to_gemini` / `GeminiSSETransformer` 等 `#[allow(dead_code)]` 函数）。

---

## 六、剩余工作清单（按执行顺序）

### 阶段 3.2：Responses 上游 adapter 落地（进行中，约 80%）

**目标**：让 4 个 ignored 测试全绿。

**当前工作树状态**：
- ✅ `protocol/responses.rs` 已写入（795 行）
- ✅ `src/types.ts` 已加 `"responses"` 到 `ApiType` 枚举 + Beta 标签
- ✅ `src/features/channels/ChannelManager.tsx` 已加 Responses 选项
- ❌ `protocol/mod.rs` 的 `get_adapter` **还没注册** responses 分支
- ❌ `handlers.rs` / `server.rs` 里的未提交改动还未确认含义
- ❌ 4 个测试的 `#[ignore]` 还没删
- ❌ 跑测试验证未做

**执行步骤**：

1. **核查 working tree 改动**：
   ```bash
   git diff src-tauri/src/proxy/handlers.rs src-tauri/src/proxy/server.rs
   ```
   判断是否是阶段 3.2 的必要改动。如是，保留；如不是，discard。

2. **在 `protocol/mod.rs` 注册 Responses**：
   - 文件顶部新增 `mod responses;`
   - `get_adapter` match 新增：`"responses" => Box::new(responses::ResponsesAdapter),`

3. **取消 4 个测试的 `#[ignore]`**：
   文件 `src-tauri/src/proxy/protocol/roundtrip_tests.rs`，行号 685 / 699 / 726 / 755。

4. **跑测试、调整**：
   ```bash
   cd src-tauri && cargo test --lib proxy::protocol::roundtrip_tests::responses_roundtrip
   ```
   如有失败，**优先调整 responses.rs 的实现匹配测试**（测试是公理的直接体现）。

5. **跑全量**：
   ```bash
   cargo test --lib
   ```
   期望：≥219 通过 / 0 失败 / 0 ignored。

6. **分两个 commit 提交**：
   - 前端 `src/types.ts` + `ChannelManager.tsx`：
     `Phase 3.2: add Responses as selectable api_type in UI`
   - Rust 端 `responses.rs` + `mod.rs` + 测试 + `handlers.rs`/`server.rs`（如需）：
     `Phase 3.2: implement Responses upstream adapter`

### 阶段 3.3：协议模块合并（10 翻译器 → 5 个文件）

**目标**：按第四节范式把每个协议的上下游代码整合到一个文件。

**步骤**（每合并一个协议一个 commit）：

#### 3.3.A Azure 合并（最简单，先做）

- 把 `azure_output.rs` 里还用到的 `azure_to_openai_request` 挪到 `azure.rs`
- 删除 `azure_output.rs`
- 更新 `mod.rs` 的 `mod` 和 `pub use`
- 调用方 `handlers.rs::handle_azure_chat` 改路径
- 加 `ENABLE_UNKNOWN_FIELD_PASSTHROUGH` 常量（Azure = OpenAI，此常量仅作一致性标记）
- 跑测试
- commit: `Phase 3.3: merge azure translation into protocol/azure.rs`

#### 3.3.B Gemini 合并

- 把 `gemini_output.rs` 里的 `gemini_to_openai_request` / `openai_to_gemini_response` 挪到 `gemini.rs`
- 已经在阶段 3.1 删了 dead code，这里只做结构合并
- 删除 `gemini_output.rs`
- 更新 `mod.rs` 和 `handlers.rs`
- commit: `Phase 3.3: merge gemini translation into protocol/gemini.rs`

#### 3.3.C Claude 合并（最大）

- 把 `claude_output.rs` 里的全部公共 API 挪到 `claude.rs`：
  - `claude_to_openai_request`
  - `openai_to_claude_response`
  - `ClaudeSSETransformer`（含 `message_delta_emitted` 字段）
  - `transform_claude_error`
- 注意：`ClaudeSSETransformer` 是下游方向翻译器，和上游方向 `transform_sse_line` 共存，不合并
- 删除 `claude_output.rs`
- 更新 `mod.rs` 和 `handlers.rs`
- commit: `Phase 3.3: merge claude translation into protocol/claude.rs`

#### 3.3.D Responses 整合

- `responses_handler.rs` 里的**纯翻译函数**移到 `protocol/responses.rs`：
  - `input_to_messages`
  - `convert_tools`
  - `passthrough_output_item`
  - `merge_tool_delta`
  - `is_function_tool_call`
- `responses_handler.rs` 只保留 HTTP 路由入口（`handle_responses` / `get_response` / `delete_response` / `cancel_response`）+ 调用 `protocol::responses` 的薄层
- SSE 重包装那段（`responses_handler.rs:820+` 约 900 行）能抽象为 `protocol::responses` 的 "OpenAI chat.completions SSE → Responses SSE" 翻译器就抽，抽不动先留在 handler 里加注释
- commit: `Phase 3.3: move Responses translation helpers to protocol/responses.rs`

### 阶段 3.4：收尾（可选）

- 清理剩余 `#[allow(dead_code)]` 标记
- 更新 `FLOW.md` 反映新结构
- 跑 `cargo clippy` 清掉 warning
- commit: `Phase 3.4: cleanup and doc refresh`

### 阶段 4：横切特性剥离成中间件

**目标**：把 forwarder 里的 `model: xxx` 注入、token 统计、熔断决策剥离成可装配的中间件链。解决 P2 / P5 / P6。

**步骤**：

1. **定义 `ForwarderMiddleware` trait**：
   ```rust
   trait ForwarderMiddleware {
       fn before_request(&self, body: &mut Value, ctx: &RequestContext);
       fn on_sse_chunk(&self, chunk: &mut Bytes, ctx: &RequestContext);
       fn after_response(&self, body: &mut Value, ctx: &RequestContext);
   }
   ```

2. **把现有横切逻辑抽成独立 middleware struct**：
   - `StreamOptionsMiddleware`：合并而不覆盖 `stream_options`（P2）
   - `ModelAnnotationMiddleware`：注入 `model: xxx`，Responses 入口不装（P5）
   - `UsageLoggingMiddleware`：token 统计
   - `CircuitBreakerMiddleware`：熔断决策
   - `IdleTimeoutMiddleware`：空闲超时（P6）

3. **P2 修复的核心改动**：
   ```rust
   // forwarder.rs:491-499 替换
   let so = body.as_object_mut().unwrap().entry("stream_options").or_insert(json!({}));
   if let Some(obj) = so.as_object_mut() {
       obj.entry("include_usage").or_insert(json!(true));
   }
   ```

4. **入口按类型装配中间件**：
   - `handle_chat_completions`、`handle_messages`：装 ModelAnnotation
   - `handle_responses`：**不装** ModelAnnotation（解决 P5）
   - 所有入口：UsageLogging、CircuitBreaker、IdleTimeout

5. **commit 粒度**：按中间件一个一个迁移，每个一个 commit。

---

## 七、恢复执行的快速启动

新会话接手时按这个顺序：

1. **读本文档到本节**
2. 确认分支和 commit 状态：
   ```bash
   git log --oneline master..HEAD
   ```
   期望：看到 10 个 commit，最新是 `907ea79`（本次文档更新可能新增一个）
3. 看 working tree：
   ```bash
   git status
   ```
   期望：4 个文件已修改 + 1 个未追踪（`responses.rs`），对应阶段 3.2 半成品
4. 跑测试确认基线：
   ```bash
   cd src-tauri && cargo test --lib 2>&1 | grep "test result"
   ```
   期望：`215 passed; 0 failed; 4 ignored`
5. 按第六节 "3.2" 节的步骤继续执行。

---

## 八、调试提示

### Responses 反向翻译的参考

- 正向逻辑（Responses → OpenAI）：`src-tauri/src/proxy/responses_handler.rs` 的 `input_to_messages`（L33）、`convert_tools`（L360）
- 反向就是把 `input[]` 事件从 `messages[]` 反构
- SSE 反向：`responses_handler.rs:820-1100` 的正向事件映射表反过来读

### 现有 adapter 模板

- **最完整**：`protocol/claude.rs` 的 `ClaudeAdapter`
- **最简单**：`protocol/openai.rs` 的 `OpenAiAdapter`（Responses 可参考这个作起点）

### 测试运行

```bash
# 跑单个模块
cargo test --lib proxy::protocol::roundtrip_tests::responses_roundtrip

# 跑全量
cargo test --lib

# 仅编译（不跑）
cargo test --lib --no-run
```

### 常见坑

1. **不要**用 `json!({ "x": ... })` 构造响应（会丢字段），用 `.clone()` + edit-in-place
2. **不要**删除未知字段——那是公理二的核心
3. **不要**破坏已绿测试——阶段 0 承诺
4. **不要**用中文 commit message（Windows PowerShell 编码会坏，作者名也会坏）
5. **不要**在 `cfg(test)` 模块外引入 `#[test]`（Rust 不允许）

---

## 九、不做什么（明确拒绝）

- 同协议直连（翻译几毫秒可接受，架构清晰更重要）
- 换中间协议（OpenAI chat.completions 继续）
- 激活 Gemini native dead code
- 引入 LiteLLM / MCP / 其他外部框架
- 流式 IR 强类型化（收益不足以匹配 800 行重写风险，**推迟到未来独立项目**）
- 给用户 UI 配置 passthrough 开关（源码常量即可）

---

## 十、提交规范

- 每阶段独立 commit 序列
- commit message 用**英文**（避免 Windows 编码问题）
- 每阶段结束跑 `cargo test --lib`，必须 pass 或只剩已知 ignored
- 任一阶段都可独立回滚
