# 代理层架构整改计划（含详细执行状态）

> **重要**：这是**带状态的路线图**。下次会话如果上下文不足、需要继续，**先读本文档**，
> 按"当前状态"节对齐进度，按"剩余任务清单"节往下执行。

---

## 零、两条公理（不可违反）

**公理一**：api-switch 是**中转和翻译器**，不是内容修改者。

**公理二**：协议以**各自官方文档**为准，这边进来什么，那边出去必须是一样的（往返无损）。

---

## 一、9 条执行决策（已全部对齐）

1. 中间格式用 OpenAI chat.completions，不换
2. OpenAI 是翻译路径，不是规范仲裁者；每个协议以自己官方文档为准
3. 当前 10 个翻译器合并成 5 个协议模块
4. 每个协议文件遵循统一范式（见下）
5. 每协议顶部一个 `ENABLE_UNKNOWN_FIELD_PASSTHROUGH` 源码常量，不做 UI 配置
6. 合并基底选择：
   - `OpenAI`：保持现状
   - `Claude`：以上游 `claude.rs` 为基底（更完整），下游方向补齐
   - `Gemini`：以下游 `gemini_output.rs` 为基底，保持 OpenAI 兼容端点方案
   - `Azure`：清理死码，合并成薄文件
   - `Responses`：以下游 `responses_handler.rs` 翻译部分为基底，**本轮补齐上游方向**，Beta 标记
   - `Custom`：保持现状
7. Responses 上游方向在阶段 3 一次做完
8. P1（Claude SSE usage=0）在阶段 3 合并重写时修掉
9. 不做：同协议直连、换中间协议、Gemini native、引入外部框架

---

## 二、协议模块统一范式

**一个协议 = 一个文件 = 一套标准套件**。所有 5 个现有协议和将来新增协议都这样：

```rust
// protocol/xxx.rs

// 1. 源码常量
const ENABLE_UNKNOWN_FIELD_PASSTHROUGH: bool = true;

// 2. 实现 ProtocolAdapter trait
pub struct XxxProtocol;
impl ProtocolAdapter for XxxProtocol {
    // URL / 鉴权
    fn build_chat_url(...) -> String;
    fn apply_auth(...);
    fn build_models_url(...);
    fn parse_models_response(...);

    // 4 个翻译方向（对称）
    fn transform_request(&self, body: &mut Value, model: &str);   // OpenAI -> 上游
    fn transform_response(&self, body: &mut Value);               // 上游 -> OpenAI
    fn transform_sse_line(&self, line: &str) -> Option<String>;   // 上游 SSE -> OpenAI SSE
    fn needs_sse_transform(&self) -> bool;
    fn extract_sse_usage(&self, line: &str) -> (i64, i64);

    // 下游方向（新加、阶段 3 统一）：
    // 现在由 handlers.rs 或 responses_handler.rs 散落调用的
    //   xxx_to_openai_request / openai_to_xxx_response / XxxSSETransformer
    // 在协议模块内作为独立 pub fn 或嵌套模块存在。
}

// 3. 单元测试：round-trip / 未知字段穿透 / 官方文档样本
```

**加第 6 个协议的成本**：复制文件、填字段、写测试、在 `mod.rs` 的 `get_adapter` 加一行、前端 `ApiType` 加选项。**不改核心**。

---

## 三、当前状态（2026-05-10 最新）

### 分支

`fix/claude-responses-proxy-issues`，从 `master` 切出，已提交 8 个 commit。

### 已完成阶段

```
✅ 阶段 0：测试基线               3 commits (bc5c5f7, d3f0300, 3cfd1e8)
✅ 阶段 1：流式公共设施           1 commit  (0771ec1)
✅ 阶段 2：字段穿透对称化         2 commits (884f738, 3980efa)
🚧 阶段 3：协议合并 + Responses 上游  1 commit 已完成 (7e450ce = P1 修复)
⏳ 阶段 4：横切特性中间件化       0 commits
```

### Commit 历史（相对 master）

```
7e450ce  Phase 3.1: fix P1 Claude streaming output_tokens always being 0
bc25d5a  tests: accept both string and array forms for Claude system field
0771ec1  Phase 1: extract shared SSE utilities into proxy::sse
3980efa  Phase 2: preserve unknown fields in Gemini translation (both directions)
884f738  Phase 2: preserve unknown fields in Claude translation (both directions)
3cfd1e8  Phase 0: add Responses adapter placeholder round-trip tests
d3f0300  Phase 0: add round-trip tests for Gemini, Azure, OpenAI, Custom
bc5c5f7  Phase 0: add Claude protocol round-trip tests as baseline
```

### 测试状态

```
215 通过 / 0 失败 / 4 ignored
```

4 个 ignored 是 `protocol/roundtrip_tests.rs` 里 `responses_roundtrip` 模块下的占位测试：
- `adapter_registered_for_responses`
- `request_openai_to_responses_upstream`
- `response_responses_to_openai_upstream`
- `upstream_unknown_field_passthrough`

**这 4 个测试的 `#[ignore]` 属性是 Responses 上游实现完成后的"启用开关"**，实现好后删除 `#[ignore]` 就行。

### 6 个原问题状态

| 问题 | 状态 | 归属 |
|---|---|---|
| P1 Claude 流式 output_tokens=0 | ✅ 已修 | 阶段 3.1 |
| P2 stream_options 无条件覆盖 | ⏳ 待修 | 阶段 4 |
| P3 UTF-8 切字 → � | ✅ 已修 | 阶段 1 |
| P4 流式 buffer 无上限 | 🟡 部分（responses_handler 内有，其他待阶段 3 整合） | 阶段 3 |
| P5 model:xxx 注入污染 Responses | ⏳ 待修 | 阶段 4 |
| P6 第二层流无 idle timeout | ⏳ 待修 | 阶段 3 或阶段 4 |

### 已部分完成的阶段 3 未提交改动

**⚠️ 注意**：当前 working tree 里有前端的改动**未提交**：
- `src/types.ts`：`ApiType` 加了 `"responses"` 选项，`API_TYPE_OPTIONS` 加了 Beta 标记，`API_TYPE_DEFAULT_URLS` 加了默认 URL
- `src/features/channels/ChannelManager.tsx`：`API_TYPES` 加了 Beta 选项

恢复工作前先跑 `git status` 确认，`git diff` 看具体改动。这些是阶段 3.2 的一部分，但 Rust 端的 `protocol/responses.rs` 还没写。

---

## 四、剩余工作清单（按执行顺序）

### 阶段 3.2：Responses 上游 adapter 实现

**目标**：让 4 个 ignored 测试全绿。

**步骤**：

1. **提交前端改动** 作为独立 commit（如果还没提交）
   ```
   commit msg: Phase 3.2: add Responses as selectable api_type in UI
   ```

2. **新建 `src-tauri/src/proxy/protocol/responses.rs`**，实现 `ProtocolAdapter` trait：
   - `build_chat_url`：`{base}/v1/responses`
   - `apply_auth`：`Bearer {api_key}`
   - `build_models_url`：`{base}/v1/models`（和 OpenAI 一致）
   - `parse_models_response`：和 OpenAI 一致
   - `transform_request`（OpenAI chat.completions → Responses 请求）：
     * `messages[]` → `input[]`（新写 `messages_to_input`，参考 `responses_handler.rs:33` 的 `input_to_messages` 反向逻辑）
     * `messages` 里的 `role: system` → 抽出到顶层 `instructions`
     * `max_tokens` → `max_output_tokens`
     * 其他字段 passthrough（按公理二）
   - `transform_response`（Responses 响应 → OpenAI chat.completions）：
     * `output[]` 里的 `message` 块 → `choices[0].message`
     * `output[]` 里的 `function_call` 块 → `choices[0].message.tool_calls`
     * `status: "completed"` → `choices[0].finish_reason: "stop"` 等
     * `usage.input_tokens` → `usage.prompt_tokens`
     * `usage.output_tokens` → `usage.completion_tokens`
     * 其他字段 passthrough
   - `needs_sse_transform`：**返回 `true`**
   - `transform_sse_line`（Responses SSE → OpenAI chat.completions SSE）：
     * `response.output_text.delta` → `choices[0].delta.content`
     * `response.function_call_arguments.delta` → `choices[0].delta.tool_calls[].function.arguments`
     * `response.completed` → `choices[0].finish_reason` + `usage`
     * `response.failed` → error 帧
     * 其他事件 → `None`（drop）
     * **注意**：这是**最难的一块**，参考 `responses_handler.rs:820+` 的正向逻辑反过来写
   - `extract_sse_usage`：从 `response.completed` 事件读 `usage.input_tokens / output_tokens`

3. **在 `protocol/mod.rs` 的 `get_adapter` 添加分支**：
   ```rust
   "responses" => Box::new(responses::ResponsesAdapter),
   ```
   并在文件顶部加 `mod responses;`

4. **取消 4 个 ignored 测试的 `#[ignore]`**，跑 `cargo test`，让它们全绿。
   如果测试断言对不上实际实现，**优先调整实现，而不是削弱断言**。

5. **提交**：
   ```
   commit msg: Phase 3.2: implement Responses upstream adapter
   ```

### 阶段 3.3：协议模块合并（10 翻译器 → 5 个文件）

**目标**：按范式二把每个协议的上下游代码整合到一个文件。

**步骤**（每合并一个协议一个 commit）：

1. **Azure 合并**（最简单，先做）
   - 把 `azure_output.rs` 里还用到的 `azure_to_openai_request` 挪到 `azure.rs`
   - 删除 `azure_output.rs`
   - 更新 `mod.rs` 里的 `mod` 和 `pub use`
   - 调用方（`handlers.rs` 的 `handle_azure_chat`）改用新路径
   - 跑测试
   - commit: `Phase 3.3: merge azure translation into protocol/azure.rs`

2. **Gemini 合并**
   - 把 `gemini_output.rs` 里还用到的 `gemini_to_openai_request` / `openai_to_gemini_response` 挪到 `gemini.rs`
   - 删除 `gemini_output.rs` 里的 dead code（`build_gemini_native_*`、`transform_request_to_gemini`、`GeminiSSETransformer` 等 `#[allow(dead_code)]` 的函数一并删）
   - 删除 `gemini_output.rs`
   - 更新 mod.rs 和调用方
   - commit: `Phase 3.3: merge gemini translation into protocol/gemini.rs`

3. **Claude 合并**（最大的一个）
   - 把 `claude_output.rs` 里的 `claude_to_openai_request` / `openai_to_claude_response` / `ClaudeSSETransformer` / `transform_claude_error` 全部挪到 `claude.rs`
   - 注意：`ClaudeSSETransformer` 本身是下游方向的翻译器，在 `claude.rs` 里它和上游方向的 `transform_sse_line` 是两个独立的 public item，不要合并
   - 删除 `claude_output.rs`
   - 更新 mod.rs 和 handlers.rs
   - commit: `Phase 3.3: merge claude translation into protocol/claude.rs`

4. **Responses 整合**
   - `responses_handler.rs` 里的 `input_to_messages` / `convert_tools` / `passthrough_output_item` / `merge_tool_delta` 等**纯翻译函数**移到 `protocol/responses.rs`
   - `responses_handler.rs` 只保留 HTTP handler 入口（`handle_responses` / `get_response` / `delete_response` / `cancel_response`）+ 调用 `protocol::responses` 的薄层
   - SSE 重包装那 900 行如果能抽象成 `protocol::responses` 里的一个 "OpenAI chat SSE → Responses SSE" 的翻译器，就抽出来；抽不出来就暂留在 handler 里，注释说明
   - commit: `Phase 3.3: move Responses translation helpers to protocol/responses.rs`

5. **每个协议文件顶部加范式常量**：
   ```rust
   /// 是否在翻译时穿透本协议官方文档未定义的字段。
   /// 默认 true：贯彻"中转不丢失"公理。
   /// 应急 false：仅传官方已知字段。如果某上游开始对未知字段返回 400，
   /// 定位到是穿透行为导致后改此常量为 false，发布新版本。
   const ENABLE_UNKNOWN_FIELD_PASSTHROUGH: bool = true;
   ```
   - **注意**：当前代码里 passthrough 行为是硬编码"总是穿透"。加常量后要把 passthrough 那段 `for (key, value) in obj.iter() { ... }` 代码包在 `if ENABLE_UNKNOWN_FIELD_PASSTHROUGH { ... }` 里
   - 5 个协议（openai / claude / gemini / azure / responses）都要加。custom 不需要（它就是 openai 的别名）
   - commit: `Phase 3.3: add ENABLE_UNKNOWN_FIELD_PASSTHROUGH guard per protocol`

### 阶段 3.4：最终收尾（可选）

- 清理所有 `#[allow(dead_code)]` 标记的函数
- 更新 `FLOW.md` 反映新结构
- 跑 `cargo clippy` 清掉剩余 warning
- commit: `Phase 3.4: cleanup and doc refresh`

### 阶段 4：横切特性剥离成中间件

**目标**：把 forwarder 里的 `model: xxx` 注入、token 统计、熔断决策剥离成可装配的中间件链。解决 P2 / P5。

**步骤**：

1. **定义 `ForwarderMiddleware` trait**（在 `forwarder/middleware.rs` 或 `forwarder.rs` 顶部）：
   ```rust
   trait ForwarderMiddleware {
       fn before_request(&self, body: &mut Value, ctx: &RequestContext);
       fn on_sse_chunk(&self, chunk: &mut Bytes, ctx: &RequestContext);
       fn after_response(&self, body: &mut Value, ctx: &RequestContext);
   }
   ```

2. **把现有横切逻辑抽成独立 middleware struct**：
   - `StreamOptionsMiddleware`：合并 stream_options 而不是覆盖（P2 修复）
   - `ModelAnnotationMiddleware`：注入 `model: xxx`（仅对非 Responses 入口启用，P5 修复）
   - `UsageLoggingMiddleware`：token 统计
   - `CircuitBreakerMiddleware`：熔断决策
   - `IdleTimeoutMiddleware`：空闲超时（P6）

3. **入口按类型装配**：
   - `handle_chat_completions`、`handle_messages` 等装 `ModelAnnotationMiddleware`
   - `handle_responses` **不装** `ModelAnnotationMiddleware`

4. **P2 修复**：`StreamOptionsMiddleware` 实现成：
   ```rust
   let so = body.as_object_mut().unwrap().entry("stream_options").or_insert(json!({}));
   if let Some(obj) = so.as_object_mut() {
       obj.entry("include_usage").or_insert(json!(true));
   }
   ```

5. **commit 粒度**：按中间件一个一个迁移，每个一个 commit。

---

## 五、恢复执行的快速清单

如果在一次新会话里接手，按这个顺序启动：

1. **先读本文档**（到这一行）
2. 跑 `git log --oneline master..HEAD` 确认分支状态
3. 跑 `git status` 看 working tree 有没有未提交改动
4. 跑 `cargo test --lib --package api-switch-lib 2>&1 | grep "test result"` 确认基线
   - 期望：215 pass / 0 fail / 4 ignored
5. 按"剩余工作清单"的**阶段 3.2 步骤 1** 开工
6. 每步 commit 完跑一次 `cargo test`
7. 每做完一个小阶段，更新本文档的"当前状态"节

---

## 六、调试提示

### 找 Responses 反向翻译的参考

- 正向逻辑（Responses → OpenAI）在 `src-tauri/src/proxy/responses_handler.rs:33-360`（`input_to_messages` 和 `convert_tools`）
- 反向就是把 `input[] items` 构造从 `messages[]` 中生成
- SSE 反向：`responses_handler.rs:820-1100` 的正向事件映射表反过来读

### 找现有 adapter 的模板

- **最完整**：`protocol/claude.rs` 的 `ClaudeAdapter`
- **最简单**：`protocol/openai.rs` 的 `OpenAiAdapter`（可以作为 Responses 的复制起点）

### 测试运行

- 跑单个协议测试：`cargo test --lib proxy::protocol::roundtrip_tests::responses_roundtrip`
- 跑全量：`cargo test --lib`
- 编译检查（不跑）：`cargo test --lib --no-run`

### 常见坑

1. **不要**用 `json!({ "x": ... })` 构造响应（会丢字段），用 `.clone()` + 修改
2. **不要**删除"未知字段"——那是公理二的核心
3. **不要**破坏已绿测试——阶段 0 承诺
4. **不要**用中文 commit message（Windows PowerShell 编码会坏）

---

## 七、不做什么（明确拒绝）

- 同协议直连（翻译几毫秒可接受，架构清晰更重要）
- 换中间协议（OpenAI chat.completions 继续）
- 激活 Gemini native dead code
- 引入 LiteLLM / MCP / 其他外部框架
- 流式 IR 强类型化（原计划阶段 3 的最后一步——收益不足以匹配 800 行重写风险，**推迟到未来独立项目**）
- 给用户 UI 配置 passthrough 开关（源码常量即可）

---

## 八、提交规范

- 每阶段独立 commit 序列
- commit message 用**英文**（避免 Windows 编码问题）
- 每阶段结束跑 `cargo test --lib`，必须 pass 或只剩已知 ignored
- 任一阶段都可独立回滚
