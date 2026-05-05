# BUG 记录

## [BUG-001] 代理核心：超时过长 + Failover 未触发 + auto 模型解析异常

**发现时间**：2026-05-02  
**发现方式**：对照 OpenCode 请求日志分析  
**状态**：待修复  
**严重程度**：高（影响用户体验，121s 卡死）  
**影响范围**：所有通过代理发出的 API 请求

---

## 1. 背景与现象

### 正常预期行为
当用户通过代理请求模型时：
1. 如果某渠道返回 502/超时，应**快速失败**或**切换备用渠道**
2. `auto` 模型应由代理解析为可用模型
3. 单请求超时应在 **5–15s** 内判定，不应等待 121s

### 实际异常行为（日志证据）

| 时间 | 渠道 | 请求模型 | 实际模型 | 耗时 | 状态 | 关键信息 |
|---|---|---|---|---|---|---|
| 21:33:48 | MORNING | auto | gpt-5.4 | 121s | ❌ 失败 | 超时，`yk.dbgai.xyz` 502 |
| 21:35:09 | xiaomi | auto | mimo-v2.5-pro | 81s | ✅ 成功 | 同一时段，另一渠道正常 |
| 21:40:38 | MORNING | auto | gpt-5.5 | 6s | ✅ 成功 | **同一渠道，换模型即正常** |
| 21:40:47 | MORNING | auto | gpt-5.5 | 6s | ✅ 成功 | 复现成功 |
| 22:53:37 | nvidia | qwen/qwen3.5-122b-a10b | qwen/qwen3.5-122b-a10b | 132s | ❌ 失败 | 流读取超时，`integrate.api.nvidia.com` 200 但 stream 断开 |

### 异常摘要
- `MORNING` 渠道在 21:33 请求 `gpt-5.4` 时，目标返回 502
- 代理**等待了 121 秒**才宣告超时失败
- 同一时间 `xiaomi` 渠道**可用且成功**
- 代理**没有触发 failover** 到可用渠道
- 2 分钟后，同一渠道请求 `gpt-5.5` **6 秒即成功**，说明网络与服务本身无持久问题
- **22:53:37 nvidia 渠道**：请求 `qwen/qwen3.5-122b-a10b`，200 状态码但流读取失败，代理**等待 132 秒**才超时，**未触发 failover**。
- **结论**：此案例进一步确认 BUG A 和 BUG B 是**系统性问题**，不仅限于 `MORNING` 渠道或 `gpt-5.4` 模型。所有通过代理的请求都面临相同风险。

---

## 2. 根因分析

### BUG A：超时配置不合理（确定性 BUG）

**现象**：单请求等待 121 秒  
**预期**：502 是即时错误，应在 5–15s 内判定失败  
**根因**：HTTP client 超时配置过长，或代理未正确处理 502 状态码

**关键日志片段**：
```
尝试路径: MORNING / gpt-5.4 [502]
...
"kind":"timeout","is_timeout":true,"is_connect":false,"is_request":true
```

502 表示上游服务明确拒绝，代理却继续等待直到 TCP/HTTP 层超时（121s），说明：
- 未对 5xx 状态码做快速失败处理
- 或未设置合理的 `connect_timeout` / `read_timeout`

---

### BUG B：Failover 逻辑未生效（确定性 BUG）

**现象**：21:33 时 `xiaomi` 渠道可用，但 `MORNING` 失败未切换  
**预期**：渠道 A 失败 → 自动尝试渠道 B  
**根因**：failover / 渠道切换逻辑在超时或 502 时未触发

**推理**：
- 21:35:09 `xiaomi` 渠道仍在正常工作（81s，成功）
- 如果 failover 正常，21:33 的失败请求应切换至 `xiaomi`
- 日志中**没有看到 fallback 尝试记录**，说明 failover 未执行

---

### BUG C：`auto` 模型解析可能有缺陷（疑似 BUG）

**现象**：`auto` → `gpt-5.4`，但 `gpt-5.4` 在该渠道不可用  
**预期**：`auto` 应解析为当前可用模型，或至少排除已知失败的模型  
**根因**：`auto` 模型映射逻辑未考虑模型可用性，或 failover 时未重新解析模型

**待确认**：
- `auto` 的解析逻辑在哪里？（代理配置？代码硬编码？）
- `gpt-5.4` 是否为有效模型名称，还是解析错误？
- 如果 `gpt-5.4` 已知不可用，为何还继续重试？

---

## 3. 待检查的代码位置

### 超时配置
- [ ] `src-tauri/src/proxy/` 下的 HTTP client 初始化代码
- [ ] 查找 `timeout`、`connect_timeout`、`read_timeout` 相关配置
- [ ] 确认 502/5xx 状态码是否触发快速失败

### Failover 逻辑
- [ ] `src-tauri/src/proxy/router.rs` 或渠道选择逻辑
- [ ] 查找 failover、retry、fallback 相关代码
- [ ] 确认渠道切换是否在错误/超时时触发

### auto 模型解析
- [ ] 查找 `auto` → 具体模型的映射逻辑
- [ ] 确认模型解析是否在每次请求时执行，还是缓存了失败结果
- [ ] 检查是否有模型可用性探测/排除机制

---

## 4. 复现步骤

1. 启动代理，确保有多个渠道配置（如 MORNING、xiaomi）
2. 向代理发送请求，模型指定为 `auto`
3. 人为让某渠道的某模型返回 502（或不可用）
4. 观察：
   - 是否等待超过 15s？
   - 是否切换至其他渠道？
   - `auto` 解析结果是否为不可用模型？

---

## 5. 修复方向（待确认）

### 短期修复
1. **降低超时**：HTTP client `timeout` 设为 15s，502 立即失败
2. **启用 failover**：渠道失败时，立即尝试下一可用渠道
3. **auto 模型排除**：记录模型失败历史，临时排除不可用模型

### 长期优化
1. **健康检查**：定期探测各渠道/模型可用性
2. **智能路由**：根据延迟、成功率动态调整渠道选择
3. **断路器**：某渠道连续失败时，暂时熔断，避免无意义等待

---

## 6. 关联日志原文

```
2026/5/2 21:33:48
MORNING
OPENCODE
gpt-5.4
121s / 5.6s 流
0	0	失败
请求模型: auto
实际模型: gpt-5.4
尝试路径: MORNING / gpt-5.4 [502]
详情
Request failed: {"stage":"send","url":"https://yk.dbgai.xyz/v1/chat/completions","kind":"timeout","is_timeout":true,"is_connect":false,"is_request":true,"is_body":false,"is_decode":false,"is_redirect":false,"source_chain":["operation timed out"],"message":"error sending request for url (https://yk.dbgai.xyz/v1/chat/completions)"}
错误
Request failed: {"stage":"send","url":"https://yk.dbgai.xyz/v1/chat/completions","kind":"timeout","is_timeout":true,"is_connect":false,"is_request":true,"is_body":false,"is_decode":false,"is_redirect":false,"source_chain":["operation timed out"],"message":"error sending request for url (https://yk.dbgai.xyz/v1/chat/completions)"}

2026/5/2 21:35:09
xiaomi
OPENCODE
mimo-v2.5-pro
81s / 5.6s 流
353089	3542	成功

2026/5/2 21:40:38
MORNING
OPENCODE
gpt-5.5
6s / 3.3s 流
0	0	成功
请求模型: auto
实际模型: gpt-5.5
尝试路径: MORNING / gpt-5.5 [200]
流结束原因: done
```

---

## 7. 修复优先级

| 优先级 | 修复项 | 理由 |
|---|---|---|
| P0 | 修复超时配置（121s → 15s） | 用户无法接受 2 分钟卡死 |
| P0 | 修复 failover 逻辑 | 同一时间有可用渠道却未切换 |
| P1 | 检查 auto 模型解析 | 避免解析为已知不可用模型 |
| P2 | 长期优化（健康检查、断路器） | 提升整体稳定性 |

---

## [BUG-002] 流被 dropped 但仍记录为成功

**发现时间**：2026-05-03  
**发现方式**：对照 OpenCode 请求日志分析  
**状态**：待修复  
**严重程度**：中高（会误判请求成功，污染成功率与日志判断）  
**影响范围**：代理流式请求日志、成功率统计、问题排查

---

## 1. 背景与现象

### 正常预期行为
当流式请求成功时：
1. 应以 `done` 或等价正常结束状态收尾
2. token 统计不应长期表现为 `0 / 0`
3. 若流被中途取消/丢弃，不应继续记为“成功”

### 实际异常行为（日志证据）

| 时间 | 渠道 | 请求模型 | 实际模型 | 耗时 | 状态 | 关键信息 |
|---|---|---|---|---|---|---|
| 01:06:47 | nvidia | qwen/qwen3.5-122b-a10b | qwen/qwen3.5-122b-a10b | 2s 流 | ✅ 成功 | `200`，但 `流结束原因: dropped`，token `0 / 0` |
| 01:15:23 | nvidia | openai/gpt-oss-120b | openai/gpt-oss-120b | 2s / 1.1s 流 | ✅ 成功 | `200`，但 `流结束原因: dropped`，token `0 / 0` |
| 01:48:30 | MORNING | auto | gpt-5.4 | 2s / 1.2s 流 | ✅ 成功 | `200`，但 `流结束原因: dropped`，token `0 / 0` |
| 01:48:30 | MORNING | gpt-5.4 | gpt-5.4 | 4s / 1.1s 流 | ✅ 成功 | 同时段再次出现短流成功，但疑似未完整消费 |
| 01:48:56 | MORNING | gpt-5.4 | gpt-5.4 | 8s / 5.4s 流 | ✅ 成功 | `251548 / 26`，更像正常完整完成 |

### 异常摘要
- 多条请求拿到 `HTTP 200` 后被记录为“成功”
- 但流结束原因不是 `done`，而是 `dropped`
- 多条日志 token 统计为 `0 / 0`
- 现象同时出现在 `nvidia` 与 `MORNING`，说明不是单一渠道问题
- `MORNING` 同时段出现“短 dropped 成功”后又出现一条更完整的成功，疑似请求被覆盖、取消或重试

---

## 2. 根因分析（初步）

### BUG A：成功判定时机过早（高概率）
**现象**：只要拿到 `HTTP 200` 就被记为成功，但流后续被丢弃  
**推测**：日志/状态统计可能在 stream 真正结束前就已经落成“成功”

### BUG B：流生命周期状态机不完整（高概率）
**现象**：`dropped` 与 “成功”同时出现  
**推测**：`dropped` 没有把最终状态降级为“取消/异常结束”

### BUG C：客户端或调用方主动取消/覆盖请求（待确认）
**现象**：同一模型短时间出现多条 dropped success，随后又出现一条更完整成功  
**推测**：调用方可能重复发起请求，旧流被中途取消，但代理仍记成功

---

## 3. 待检查的代码位置

### 流结束原因打点
- [ ] `src-tauri/src/proxy/forwarder.rs`
- [ ] 查找 `dropped`、`done`、stream close、client disconnect 相关逻辑
- [ ] 确认 `dropped` 是谁打出来的

### 成功状态判定
- [ ] 查找“成功”日志/状态是在哪个时机写入
- [ ] 确认是否在收到 `HTTP 200` 后就提前落成功
- [ ] 确认 stream 完整结束前是否允许状态回写

### 客户端取消/覆盖行为
- [ ] 查 OPENCODE 调用层是否会主动取消旧流
- [ ] 查是否存在并发请求覆盖同一日志项/会话项
- [ ] 查是否有自动重试逻辑导致短流 dropped

---

## 4. 修复方向（待确认）

### 短期修复
1. `dropped` 不再计为“成功”
2. 成功状态应在 stream 正常结束后才落库/落日志
3. 若客户端主动取消，应区分为 `cancelled` / `dropped`，不能混入成功率

### 长期优化
1. 完善流生命周期状态机：`started -> streaming -> done/cancelled/dropped/error`
2. 成功率统计只统计 `done`
3. 为 dropped 增加原因分类（客户端取消、上游断流、本地覆盖等）

---

## 5. 修复优先级

| 优先级 | 修复项 | 理由 |
|---|---|---|
| P0 | 修正 dropped 仍记成功 | 会直接污染成功率与故障判断 |
| P1 | 明确客户端取消与上游断流分类 | 方便定位真实原因 |
| P1 | 检查重复请求/覆盖行为 | MORNING 同时段现象可疑 |
| P2 | 完善流状态机 | 提升长期可观测性 |

---

*记录人：Sisyphus*  
*记录时间：2026-05-03*

---

## [BUG-003] 设置版本信息未随程序版本升级同步更新

**发现时间**：2026-05-03  
**发现方式**：使用过程中人工发现  
**状态**：待修复  
**严重程度**：中（影响版本识别、迁移判断、设置兼容处理）  
**影响范围**：配置版本识别、设置迁移、后续升级兼容逻辑

---

## 1. 背景与现象

### 正常预期行为
当程序版本升级时：
1. 设置中的版本信息应同步更新
2. 程序应能根据设置版本判断是否需要迁移
3. 配置与当前程序版本应保持一致，避免“旧配置被误认为新配置”

### 实际异常行为
- 程序版本升级后，设置中的版本信息没有随之更新
- 后续如果依赖设置版本做迁移/兼容判断，可能拿到过期版本号
- 这会让“配置版本”和“程序版本”脱钩

### 异常摘要
- 版本信息没有跟随程序升级自动同步
- 未来一旦引入更复杂的设置迁移逻辑，容易出现错误判断
- 当前应尽早补上版本同步机制，避免后续扩展时埋坑

---

## 2. 初步判断

### BUG A：缺少程序版本 → 设置版本的同步写回
**现象**：程序升级后，设置版本保持旧值  
**推测**：启动时没有比较当前程序版本与设置版本，也没有写回最新版本

### BUG B：缺少统一的配置迁移入口
**现象**：设置版本信息即使存在，也没有明显的升级迁移闭环  
**推测**：配置初始化 / 启动流程里没有统一的“版本检查 → 迁移 → 写回”步骤

---

## 3. 待检查的代码位置

### 设置版本定义与读写
- [ ] `src-tauri/src/database/dao/config_dao.rs`
- [ ] 查找 settings 中是否已有版本字段
- [ ] 确认版本字段当前由谁写入

### 程序启动流程
- [ ] `src-tauri/src/lib.rs`
- [ ] `src-tauri/src/commands/config.rs`
- [ ] 查找启动时是否有配置迁移/版本同步逻辑

### 升级检查相关逻辑
- [ ] 查找版本号来源（如 `CARGO_PKG_VERSION`）
- [ ] 确认是否存在“当前程序版本 != 设置版本”时的处理分支

---

## 4. 修复方向（待确认）

### 短期修复
1. 启动时读取当前程序版本与设置版本
2. 若不一致，执行必要迁移后写回最新版本
3. 保证设置版本始终反映当前程序已完成的迁移状态

### 长期优化
1. 建立统一的 settings migration 入口
2. 为每个版本升级定义明确的迁移步骤
3. 让版本升级与配置兼容逻辑可追踪、可测试

---

## 5. 修复优先级

| 优先级 | 修复项 | 理由 |
|---|---|---|
| P1 | 启动时同步设置版本 | 避免版本信息长期滞后 |
| P1 | 建立迁移入口 | 为后续配置结构演进打基础 |
| P2 | 增加版本迁移测试 | 防止后续升级回归 |

---

*记录人：Sisyphus*  
*记录时间：2026-05-03*

---

## [BUG-004] LOG.MD 深度分析：既有BUG代码验证 + 新发现

**发现时间**：2026-05-04  
**发现方式**：对照 `forwarder.rs` / `router.rs` / `circuit_breaker.rs` / `server.rs` 源码逐条验证 LOG.MD  
**状态**：分析完成，待修复  

---

## 一、对 BUG-001 的代码验证

### ✅ BUG A "超时配置不合理" — 部分正确

**代码证据** (`server.rs` L1723)：
```rust
http_client: reqwest::Client::builder()
    .connect_timeout(Duration::from_secs(connect_timeout_secs))  // 默认30s，clamp 1-300
    .read_timeout(Duration::from_secs(120))                      // ← 硬编码120s
    .build()
```

- `read_timeout=120s` 硬编码，不可配置，解释了日志中 `121s` 超时
- `connect_timeout` 来自 `settings.proxy_connect_timeout_secs`，可配置
- **结论**：120s read_timeout 确实过长，但不是 BUG-001 描述的"未设置 timeout"，而是**硬编码值过大**

### ❌ BUG B "Failover 未触发" — **错误判断**

**代码证据** (`forwarder.rs` `forward_with_retry`)：
```rust
for entry in entries {
    // Check circuit breaker
    if !cb.is_available() { continue; }
    
    match forward_single(...).await {
        Ok(result) => return Ok(result.response),  // 成功则返回
        Err((e, status)) => {
            push_attempt(&mut attempts, entry, log_status, false, Some(e.clone()));
            cool_down_entry(state, entry).await;    // ← 失败后冷却
            last_error = Some((e, status));
            continue;                               // ← 继续尝试下一 entry
        }
    }
}
```

- Failover 逻辑**完全正确**：失败 → 冷却 → continue → 尝试下一 entry
- 日志中 `尝试路径: MORNING / gpt-5.4 [502]` 只有一个条目，说明**当时只有这一个可用 entry**（其他 entry 可能已被 circuit_breaker 跳过，或同 model 没有其他渠道）
- **结论**：Failover 代码没问题，问题是可用 entry 不足

### ❌ BUG C "auto 模型解析异常" — **错误判断**

**代码证据** (`router.rs`)：
```rust
let (model, mode) = parse_model_preference(requested_model);
// mode == Auto 时：
let auto_entries = filter_available(entries);  // 过滤 enabled + 非 circuit-open
```

- `auto` 模式正确：返回所有 enabled entry，按 priority 排序
- `gpt-5.4` 是 MORNING 渠道的实际模型，不是 auto 解析错误
- **结论**：auto 解析逻辑正确，此 BUG 不存在

### 📋 BUG-001 修正后的真实问题

| 问题 | 真实状态 | 根因 |
|------|----------|------|
| 超时过长 | ✅ 确认 | `read_timeout` 硬编码 120s |
| Failover 未触发 | ❌ 误判 | Failover 正常，只是当时无其他可用 entry |
| auto 解析异常 | ❌ 误判 | auto 解析正确，`gpt-5.4` 是正常解析结果 |

---

## 二、对 BUG-002 的代码验证

### ✅ "dropped 记为成功" — **完全正确**

**代码证据** (`forwarder.rs` `StreamLogGuard::Drop`)：
```rust
impl Drop for StreamLogGuard {
    fn drop(&mut self) {
        if !self.logged.swap(true, Ordering::SeqCst) {
            // Client disconnect: upstream was fine (status_code 200), mark as success.
            // stream_end_reason: dropped distinguishes it from normal completion.
            ...
            log_usage(
                ..., true, ...,  // ← success = true
                Some(StreamEndReason::Dropped),
            );
        }
    }
}
```

- 代码注释明确写了 "Client disconnect: upstream was fine (status_code 200), mark as success"
- **设计意图**：客户端断开 ≠ 上游失败，所以标记为成功
- **实际问题**：客户端断开 = 用户没收到完整响应，不应算成功；且不触发冷却

### 🆕 BUG-002 遗漏的关键问题：dropped 不触发冷却

`StreamLogGuard::Drop` 只记录日志，**不调用 `cool_down_entry`**。对比 `forward_with_retry` 中的失败路径：
```rust
Err((e, status)) => {
    cool_down_entry(state, entry).await;  // ← 失败触发冷却
    continue;
}
```

而 `StreamLogGuard::Drop` 里只有 `log_usage`，没有 `cool_down_entry`。这意味着：
- 客户端断开的 entry 不会被冷却
- 下次请求可能又走同一条路，然后又超时断开
- 形成 **"超时 → dropped → 不冷却 → 再超时"** 的死循环

### 🆕 BUG-002 遗漏的关键问题：0 0 成功大量出现

LOG.MD 中多条 `0 0 成功` 日志（prompt_tokens=0, completion_tokens=0）：
- 第109-110、287-288、309-310、357、379、391 行
- 这些是客户端在收到任何 chunk 前就断开的场景
- `StreamLogGuard::Drop` 仍用 `success=true` 记录，token=0

---

## 三、对 BUG-003 的验证

### ✅ 设置版本未同步 — **正确，但与 LOG.MD 无关**

这是独立的配置管理问题，不影响代理请求处理，此处不展开。

---

## 四、LOG.MD 新发现

### 🆕 发现 1：yk.dbgai.xyz 频繁超时（上游问题）

| 时间 | 关键信息 |
|------|----------|
| 多次 | `is_connect:true, is_request:true, is_body:false` |

- TCP 连接成功，请求阶段超时，上游接收了请求但没响应
- 这是 MORNING 渠道上游 `yk.dbgai.xyz` 的服务器问题
- **API-Switch 应对此做快速失败**（read_timeout 120s 太长）

### 🆕 发现 2：nvidia 连接被中止

| 时间 | 关键信息 |
|------|----------|
| 01:06:47 | `os error 10053` (WSAECONNABORTED) |

- 可能原因：本地防火墙/杀毒软件干扰，或 nvidia 服务器主动断开
- 偶发性，非系统性问题

### 🆕 发现 3：sensenova token 超限

| 时间 | 关键信息 |
|------|----------|
| 01:23:xx | `Requested 267277 tokens, exceeding limit of 262144` |

- 正常的上游拒绝（400），可在请求前检查 token 数量避免浪费请求

### 🆕 发现 4：first_token_ms 极高（30s+）

| 时间 | 关键信息 |
|------|----------|
| 01:51:xx | `first_token_ms:30729` (30.7s) |

- 上游模型推理慢，非 API-Switch 问题
- 但可考虑对 `first_token_ms` 设置告警阈值

### 🆕 发现 5：同一时间戳重复请求

| 时间 | 关键信息 |
|------|----------|
| 01:48:30 | 两个完全相同的请求都"成功"但 0 token |

- 可能是 OpenCode 客户端的重试机制导致
- 两个请求都被标记为成功，进一步污染统计数据

---

## 五、综合修复方案

### P0 — 必须立即修复

#### 1. `dropped` 不应标记为成功（见 BUG-005 §6，统一处理）

**问题**：`StreamLogGuard::Drop` 中 success=true，0 token 的 dropped 也记为成功，污染统计 + 不触发冷却。

**修法**：与 BUG-005 §6 统一，completion_tokens > 0 才算成功。

#### 2. `dropped` 应触发冷却

**文件**：`src-tauri/src/proxy/forwarder.rs` `StreamLogGuard::Drop`

**修改方案**：
```rust
// 在 Drop 中加入冷却逻辑
if self.completion_tokens.load(Ordering::SeqCst) == 0 {
    // 0 token 的 dropped 说明上游有问题，应冷却
    let state = self.state.clone();  // 需要在 guard 中持有 state 引用
    let entry = self.entry.clone();
    tokio::spawn(async move {
        cool_down_entry(&state, &entry).await;
    });
}
```

**注意**：这需要在 `StreamLogGuard` 中增加 `state: Arc<ProxyState>` 字段。

#### 2. 降低流式超时（read_timeout → first_token_timeout）

**文件**：`src-tauri/src/proxy/forwarder.rs` / `server.rs`

**当前问题**：`read_timeout=120s` 硬编码，长链思考时等 2 分钟卡死。

**修法**：取消全局 read_timeout 一刀切，改为流式分阶段超时：

| 阶段 | 超时 | 来源 |
|------|------|------|
| 等首 token | **30s**（可配置） | 新增 `first_token_timeout_secs` |
| token 间隔 | 已有 idle_timeout | 现有逻辑 |
| 整体兜底 | 保留 read_timeout 120s | 防极端情况 |

```rust
// server.rs: read_timeout 保留 120s 作为最终兜底
// forwarder.rs: 新增 first_token_timeout
let first_token_deadline = sleep(Duration::from_secs(
    settings.first_token_timeout_secs.unwrap_or(30)
));
```

**注意**：用户场景基本全为流式，非流暂无需求。

### P1 — 近期修复

#### 4. 流式请求增加 first_token 超时

**文件**：`src-tauri/src/proxy/forwarder.rs` `body_stream` closure

**修改方案**：在 `tokio::select!` 中增加 `first_token_deadline`：
```rust
// 如果 30s 内没收到第一个 token，标记为超时
let first_token_deadline = sleep(Duration::from_secs(30));
tokio::pin!(first_token_deadline);

tokio::select! {
    chunk = upstream.next() => { ... }
    _ = &mut first_token_deadline => {
        // 标记为 first_token_timeout，触发冷却
    }
    _ = idle_timeout => { ... }
}
```

#### 5. 请求前检查 token 数量

**文件**：`src-tauri/src/proxy/forwarder.rs` `forward_single`

**修改方案**：在发送请求前估算 token 数，超过上游限制时提前返回错误。

### P2 — 长期优化

#### 6. 成功率统计排除 dropped

前端/统计层只统计 `stream_end_reason == done` 的请求为成功。

#### 7. 为 dropped 增加子分类

- `dropped_by_client`：客户端主动取消
- `dropped_by_upstream`：上游断流
- `dropped_by_timeout`：流式超时

---

## 六、修复优先级汇总

| 优先级 | 修复项 | 影响 | 工作量 |
|--------|--------|------|--------|
| **P0** | dropped 标记为失败（或条件判定） | 污染成功率 | 小 |
| **P0** | dropped 触发冷却 | 死循环超时 | 中（需改 guard 结构） |
| **P0** | read_timeout 120s → 60s | 卡死 2 分钟 | 极小 |
| **P1** | first_token 超时（30s） | 上游慢响应 | 中 |
| **P1** | 请求前 token 数检查 | 浪费请求 | 小 |
| **P2** | 成功率统计排除 dropped | 数据不准 | 小 |
| **P2** | dropped 子分类 | 排查困难 | 中 |

---

*记录人：Sisyphus*  
*记录时间：2026-05-04*

---

## [BUG-005] 成功 0/0 打断模型循环（根因分析 v2）

**发现时间**：2026-05-04  
**发现方式**：LOG.MD 中 9 条 `0 / 0 / 成功` 记录 + `forwarder.rs` 全链路代码追踪 + 具体案例诊断数据验证  
**状态**：待修复  
**严重程度**：**极高**（直接打断 OpenCode 模型循环，用户体感"请求坏了"）  
**影响范围**：所有流式请求

---

## 1. 典型案例

```
时间：2026/5/3 11:55:00
渠道：MORNING / 模型：gpt-5.5 / 请求模型：auto
耗时：26s / 23.0s 流
IN/OUT：0 / 0 / 成功
stream_end_reason：done

Stream diagnostic:
  chunk_count: 2
  streamed_bytes: 446
  first_token_ms: 23007
  elapsed_ms: 25152
  status_code: 200
  error: null
```

**关键数据**：上游发了 **2 个 chunk、446 字节**，**不是空 body**。流正常结束（done），无错误。但 token 计数为 0。

---

## 2. 根因：Streaming 响应中 `usage` 字段为 null

### 2.1 代码证据

`extract_usage_tokens`（`forwarder.rs`）：

```rust
fn extract_usage_tokens(body: &Value) -> (i64, i64) {
    let usage = body.get("usage");
    let prompt_tokens = usage
        .and_then(|v| v.get("prompt_tokens"))
        .and_then(Value::as_i64)
        .unwrap_or(0);    // ← usage=null 时走这里，返回 0
    let completion_tokens = usage
        .and_then(|v| v.get("completion_tokens"))
        .and_then(Value::as_i64)
        .unwrap_or(0);    // ← 同上
    (prompt_tokens, completion_tokens)
}
```

`append_and_parse_sse` 只解析 `usage` 字段，不计数实际 content tokens。

### 2.2 OpenAI 流式响应的实际情况

**OpenAI 兼容 API 的流式 chunk 默认 `usage: null`**：

```
data: {"id":"chatcmpl-xxx","choices":[{"delta":{"content":"你好"},"finish_reason":null}],"usage":null}

data: {"id":"chatcmpl-xxx","choices":[],"finish_reason":"stop","usage":null}

data: [DONE]
```

只有请求时传了 `stream_options: {"include_usage": true}`，最后一个 chunk 才带真实 usage：

```
data: {"choices":[],"usage":{"prompt_tokens":123,"completion_tokens":456}}
```

**当前代码构建请求时没有加 `stream_options`**，所以所有流式请求的 token 计数都依赖上游是否主动返回 usage —— 大多数上游不返回。

### 2.3 那 446 字节是什么？

两种可能：

| 可能性 | 内容 | 客户端行为 |
|--------|------|-----------|
| **A. 正常空内容** | `delta.content=""` + `finish_reason=stop` + `usage=null` | 收到空回复，认为"模型没说话" |
| **B. 上游错误** | `{"error":{"message":"Rate limit","type":"rate_limit_error"}}` | 收到错误，认为请求失败 |

**23 秒的 first_token** 强烈暗示 **可能性 B** —— 上游排队 23 秒后返回了错误。

### 2.4 客户端为什么打断循环

```
客户端 (OpenCode) 视角：
  ① 发送流式请求
  ② 收到 200 OK + Content-Type: text/event-stream
  ③ 读到 2 个 chunk (446 字节)
  ④ chunk 中要么是空内容，要么是错误信息
  ⑤ 没有有效 completion → 打断模型循环
```

**代理视角与客户端视角的错位**：

```
代理：收到 200 + 流正常结束 → 标记"成功"
客户端：收到 200 + 但无有效内容 → 认为"失败"
```

---

## 3. 之前的误判（BUG-005 v1）

| v1 判断 | v2 修正 | 证据 |
|---------|---------|------|
| 上游返回空 body | 上游返回了 446 字节（2 chunks） | `chunk_count=2, streamed_bytes=446` |
| 客户端收到 0 字节 | 客户端收到了数据，但无效 | 诊断数据 |
| 问题在客户端解析 | **问题在代理：没请求 usage + 没检测错误** | 代码分析 |

---

## 4. 修复方案（按优先级）

### 方案 ① 流式请求加入 `stream_options`（治本，P0）

**文件**：`src-tauri/src/proxy/forwarder.rs` `forward_single`

```rust
// 构建请求体时，对流式请求加入 stream_options
if is_stream {
    // 让上游在最后一个 chunk 返回真实 token usage
    body_obj["stream_options"] = serde_json::json!({"include_usage": true});
}
```

**效果**：上游最后一个 chunk 带 `usage: {prompt_tokens, completion_tokens}` → token 计数准确 → 不再出现"有数据但 0 token"。

**兼容性**：不支持 `stream_options` 的上游会忽略此字段，不影响正常响应。

### 方案 ② SSE 错误检测（P0，新增）

**文件**：`forwarder.rs` `append_and_parse_sse`

```rust
fn append_and_parse_sse(
    buffer: &mut String,
    chunk: &Bytes,
    prompt_tokens: &Arc<AtomicI64>,
    completion_tokens: &Arc<AtomicI64>,
    has_error: &Arc<AtomicBool>,           // 新增
    error_message: &Arc<Mutex<String>>,    // 新增
) {
    buffer.push_str(&String::from_utf8_lossy(chunk));
    while let Some(line_end) = buffer.find('\n') {
        let mut line = buffer.drain(..=line_end).collect::<String>();
        if line.ends_with('\n') { line.pop(); }
        if line.ends_with('\r') { line.pop(); }
        let Some(payload) = line.strip_prefix("data: ") else { continue };
        if payload == "[DONE]" { continue }
        let Ok(value) = serde_json::from_str::<Value>(payload) else { continue };

        // 新增：检测上游错误
        if let Some(error) = value.get("error") {
            has_error.store(true, Ordering::Relaxed);
            if let Some(msg) = error.get("message").and_then(|v| v.as_str()) {
                *error_message.lock().unwrap() = msg.to_string();
            }
            continue;
        }

        let (prompt, completion) = extract_usage_tokens(&value);
        if prompt > 0 { prompt_tokens.store(prompt, Ordering::Relaxed); }
        if completion > 0 { completion_tokens.store(completion, Ordering::Relaxed); }
    }
}
```

**效果**：上游返回 `{"error":{...}}` 时，代理能识别为失败，不再误标"成功"。

### 方案 ③ 0-token done 标记为失败 + 触发冷却（P0，兜底）

**文件**：`forwarder.rs` `body_stream` closure 的 `Poll::Ready(None)` 分支

```rust
Poll::Ready(None) => {
    if !logged.swap(true, Ordering::SeqCst) {
        let pt = prompt_tokens.load(Ordering::SeqCst);
        let ct = completion_tokens.load(Ordering::SeqCst);
        let err = has_error.load(Ordering::SeqCst);  // 来自方案②

        // 兜底：0 token 且无错误标记的 done，也不是真正的成功
        let is_real_success = ct > 0 || (pt > 0 && !err);

        log_usage(
            &db2, &ah2, ak2.as_ref(), &e2, &rm2,
            is_real_success,  // ← 0 token → false
            pt, ct, ft, lat,
            sc, true, Some(stream_summary.as_str()),
            Some(attempt_path.as_str()), Some(StreamEndReason::Done),
        );

        if !is_real_success {
            spawn_cool_down(scb, sfc, sdb, db2.clone(), eah, eid);
        } else {
            spawn_record_circuit_success(scb, sfc, sdb, db2.clone(), eah, eid);
        }
    }
    Poll::Ready(None)
}
```

**效果**：
- 日志不再显示"成功"（用户不再看到 0/0/成功）
- 触发冷却 → 下次请求自动 failover 到备用渠道
- 不会反复命中同一个"空响应"渠道

### 方案 ④ 流结束时注入 [DONE]（P1，防御性）

如果上游没发 `[DONE]`（异常情况），代理在 `Poll::Ready(None)` 前注入：

```rust
// 用 need_done_inject 标志，在返回 None 前先返回 [DONE]
if need_done_inject.compare_exchange(false, true, ...).is_ok() {
    return Poll::Ready(Some(Ok(Bytes::from_static(b"data: [DONE]\n\n"))));
}
```

**优先级低于 ①②③**：加了 `stream_options` 后，正常上游都会发 `[DONE]`。

---

## 5. 修复优先级

| 优先级 | 修复项 | 解决的问题 | 工作量 |
|--------|--------|-----------|--------|
| **P0** | ① `stream_options: {include_usage: true}` | **根治**：token 计数准确 | 极小（1 行） |
| **P0** | ② SSE 错误检测 | 上游错误不误标成功 | 小（+10 行） |
| **P0** | ③ 0-token done 标记失败 + 冷却 | 兜底 + 触发 failover | 小（改 1 处） |
| **P0** | ③ 同时处理 dropped 的冷却 | 死循环超时 | 中（改 guard 结构） |
| **P0** | read_timeout 120s → 60s | 卡死 2 分钟 | 极小 |
| **P1** | ④ 注入 [DONE] | 防御性，上游异常时兜底 | 中 |
| **P1** | first_token 超时（30s） | 上游慢响应 | 中 |
| **P1** | 请求前 token 数检查 | 浪费请求 | 小 |
| **P2** | 成功率统计排除 dropped | 数据不准 | 小 |
| **P2** | dropped 子分类 | 排查困难 | 中 |

**推荐执行顺序**：① → ② → ③ → read_timeout，共改约 30 行代码。

---

## 6. 最小修复（大修期间临时方案）

**核心原则（黄金法则）**：  
**不管流结束原因是 `done` 还是 `dropped`，只要 `IN=0` 且 `OUT=0`，一律判定为失败。**  
- **理由**：下游没收到任何有效内容 = 请求失败。  
- **后果**：标为失败 → 触发冷却 → 下次请求自动 failover 到备用渠道。  
- **统一标准**：`success = (prompt_tokens > 0 || completion_tokens > 0)`，不再区分结束原因。  
- **0/0 成功比错误更致命**：上游报错时，下游看到 `error` → 触发重试/failover → 继续工作；但 0/0 成功时，下游看到 `success` + 空内容 → 认为请求已完成 → 不知道该重试 → **直接终止循环**。错误是"明确的失败"，下游知道怎么处理；0/0 成功是"虚假的成功"，下游被骗了，只能停摆。  
- **正常情况下 0/0 不应该存在**：`prompt_tokens > 0`（用户有输入）且 `completion_tokens > 0`（模型有输出）。0/0 只在异常时出现（流断开/上游错误/未请求 usage）。当前 0/0 泛滥是因为没发 `stream_options: {include_usage: true}`，导致上游不返回 usage → token 永远为 0。P1 修复后 0/0 会大幅减少。

**涉及两处代码**：

### 6.1 正常流结束（Poll::Ready(None)）

**文件**：`forwarder.rs` L697 附近

```rust
// 原来：
let success = status_code == 200 && error.is_none();

// 改为：
let success = status_code == 200 && error.is_none()
    && (prompt_tokens.load(SeqCst) > 0 || completion_tokens.load(SeqCst) > 0);
```

### 6.2 流被 Dropped（客户端断开 / 上游中断）

**文件**：`forwarder.rs` L252-278 `StreamLogGuard::Drop`

```rust
// 原来（L258, L274）：
log_usage(..., true, ..., Some(StreamEndReason::Dropped),);

// 改为：
let pt = prompt_tokens.load(Ordering::SeqCst);
let ct = completion_tokens.load(Ordering::SeqCst);
let success = status_code == 200 && (pt > 0 || ct > 0);
log_usage(&db, &app_handle, access_key.as_ref(), &entry, &requested_model,
    success, pt, ct, first_token_ms, latency_ms, status_code,
    true, None, Some(attempt_path.as_str()), Some(StreamEndReason::Dropped));
```

**效果**：
- 0 token 的 dropped → `success = false` → 进入冷却计数 → 下次 failover
- 有实际 token 的 dropped → `success = true`（客户端主动断开，但上游已返回数据）
- 不影响现有流式管线结构，不改 `StreamLogGuard` 生命周期

**风险**：极低。0 token 请求本身就是无用响应，标为失败比标为成功更准确。

---

*记录人：Sisyphus*  
*记录时间：2026-05-04（v2 修正：基于具体案例诊断数据，修正"空 body"误判）*

---

## 7. 数据库全量扫描验证（v3，2026-05-04）

**扫描范围**：`D:\OneDrive\API-Switch\api-switch.db` 全部 11,677 条 usage_logs  
**目的**：验证 §6 修复覆盖范围，排查遗漏

### 7.1 总览

| 指标 | 数量 |
|------|------|
| 总记录 | 11,677 |
| 成功 | 10,788 (92.4%) |
| 失败 | 889 (7.6%) |
| **0/0 成功** | **117 (占成功 1.1%)** |

### 7.2 0/0 成功按 stream_end_reason 分类

| reason | 数量 | 根因 | §6是否覆盖 |
|--------|------|------|-----------|
| **done** | **85** | 流正常结束，但 usage=null → token=0 | ✅ §6.1 token判定覆盖 |
| **dropped** | **26** | 客户端/上游断开，token=0 | ✅ §6.2 已处理 |
| **unknown** | **6** | nvidia早期记录，other字段无reason | ✅ §6.1 覆盖（done路径） |

### 7.3 🔴 关键发现：done + 0/0 是最大头（85条）

之前 LOG.MD 只看到 9 条，实际 DB 里有 **85 条 done + 0/0**，占 0/0 成功的 **72.6%**。

**content 字段分析**（85条 done）：
- **70条有实际内容**（Stream diagnostic JSON，约 537-549 字节）→ 流确实传输了数据，但 usage=null 导致 token 为 0
- 15条 content 为空

**渠道分布**：

| 渠道 | 数量 | 说明 |
|------|------|------|
| MORNING | 78 | gpt-5.5 为主，上游不返回 usage |
| iqach.top | 9 | mimo-v2.5/mimo-v2.5-pro |
| nvidia | 4 | openai/gpt-oss-120b |
| 其他 | - | 少量 |

**first_token_ms 分布**：

| 区间 | 数量 | 说明 |
|------|------|------|
| ≤2s 正常 | 12 | 正常首token，但 usage=null |
| 2-5s 偏慢 | 49 | 大量集中在此区间 |
| 5-10s 慢 | 26 | |
| 10-30s 很慢 | 13 | |
| 30s+ 超慢 | 10 | 上游可能排队/重试 |
| 无数据 | 7 | nvidia早期 |

**结论**：done + 0/0 的 85 条全部是 **usage=null 导致 token 计数为 0**，上游实际返回了数据（有 content），但代理没拿到 token。§6.1 的 token 判定（`ct > 0 || pt > 0`）能正确将这些标为失败 → 触发冷却 → failover。

### 7.4 ⚠️ 发现 dropped 成功/失败判断不一致

| 标记 | 数量 | 说明 |
|------|------|------|
| dropped → success=**false** | **105** | 在 error_message 中被记录 |
| dropped → success=**true** | **26** | 0/0 成功中被统计 |

同一种 stream_end_reason（dropped），有 105 条被标为失败，26 条被标为成功。

**可能原因**：代码中有两条路径处理 dropped：
1. `forward_with_retry` 中请求发送失败 → 直接标失败（105条）
2. `StreamLogGuard::Drop` 中流式阶段断开 → 原来标成功（26条）

§6.2 修复后，路径 2 也会根据 token 数判定，dropped + 0/0 → 失败，与路径 1 统一。

### 7.5 失败记录分类（889条）

| 错误类型 | 数量 | 说明 |
|----------|------|------|
| stream dropped before normal completion | 105 | 流式中断 |
| upstream 404 | ~140 | 模型/路由不存在 |
| 请求发送失败（网络） | ~80 | DNS/连接问题 |
| auth_error (401/403) | 115 | 认证失败 |
| timeout | 93 | 超时 |
| upstream 500/502/503 | ~40 | 上游服务端错误 |
| upstream 400 | ~30 | 请求格式错误 |
| 其他 | ~86 | error decoding/empty 等 |

### 7.6 §6 修复覆盖度验证

| 场景 | DB中数量 | §6.1 (done路径) | §6.2 (dropped路径) | 是否完全覆盖 |
|------|---------|----------------|-------------------|------------|
| done + 0/0 | 85 | ✅ token判定→失败 | - | ✅ |
| dropped + 0/0 | 26 | - | ✅ token判定→失败 | ✅ |
| unknown + 0/0 | 6 | ✅ 走done路径 | - | ✅ |
| **合计 0/0 成功** | **117** | **91** | **26** | **✅ 100%** |

**结论**：§6 的两处修复（done 路径 + dropped 路径）覆盖了全部 117 条 0/0 成功场景。无需额外新增代码路径。

---

*记录人：Sisyphus*  
*记录时间：2026-05-04（v3：数据库全量扫描验证，确认§6覆盖度 + 发现done+0/0为最大类别）*

---

*记录人：Sisyphus*  
*记录时间：2026-05-02*
