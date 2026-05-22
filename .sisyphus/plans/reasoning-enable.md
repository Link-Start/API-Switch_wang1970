# 启用 Reasoning 思维链功能

## TL;DR

> **目标**: 移除 reasoning 控制字段的全面剥离，采用"下游发起"策略，让思维链功能正常工作。
> 
> **核心修改**: 移除 4 个位置的 `strip_downstream_reasoning_request` 调用
> 
> **预计工作量**: Short (2-4h)
> **并行执行**: YES - 2 波
> **关键路径**: handlers.rs → forwarder.rs → 测试验证

---

## Context

### 原始需求

用户要求保证思维链能正常使用，不出现被上游终止的情况。

### 分析发现

通过 `docs/openai-reasoning-analysis.md` 第12节的详细分析，发现：

1. 当前代码在 **5 个位置** 调用 strip 函数，完全剥离 reasoning 控制字段
2. 测试已证明 `reasoning_effort` 对非推理模型安全（被忽略，不报错）
3. 采用"下游发起"策略：下游发起思维链，我们按思维链方式走；下游不发起思维链，我们也要保证正常

### 评审结论

Oracle + Metis 评审通过，确认策略正确，有 2 个补充点：
- 保留 responses_handler.rs:87-96 的 Responses 专属字段剥离
- 验证转换函数对格式专属结构的处理

---

## Work Objectives

### Core Objective

移除 reasoning 控制字段的全面剥离，让思维链功能正常工作。

### Concrete Deliverables

- 移除 handlers.rs:250 的 strip 调用
- 移除 handlers.rs:327 的 strip 调用
- 移除 responses_handler.rs:85 的 strip 调用
- 移除 forwarder.rs:679-683 的 strip 调用
- 保留 responses_handler.rs:87-96 的 Responses 专属字段剥离
- 更新测试断言
- 验证转换函数对格式专属结构的处理

### Definition of Done

- [ ] `cargo build` 无错误
- [ ] `cargo test` 无失败
- [ ] T1-T7 测试用例全部通过

### Must Have

- 移除 4 个位置的 strip 调用
- 保留 responses_handler.rs:87-96 的 Responses 专属字段剥离
- 测试通过

### Must NOT Have (Guardrails)

- 不修改转换函数逻辑（claude_to_openai_request, responses_to_openai_chat_request）
- 不添加新的 reasoning 处理逻辑
- 不修改 normalize_reasoning_fields 函数

---

## Verification Strategy

### Test Decision

- **Infrastructure exists**: YES
- **Automated tests**: YES (Tests-after)
- **Framework**: cargo test
- **Agent-Executed QA**: YES

### QA Policy

每个任务完成后执行 `cargo test` 验证。
最终执行 12.10 节的 7 个测试用例。

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (移除 strip 调用):
├── Task 1: 移除 handlers.rs:250 的 strip 调用 [quick]
├── Task 2: 移除 handlers.rs:327 的 strip 调用 [quick]
├── Task 3: 移除 responses_handler.rs:85 的 strip 调用 [quick]
└── Task 4: 移除 forwarder.rs:679-683 的 strip 调用 [quick]

Wave 2 (验证和测试):
├── Task 5: 验证转换函数对格式专属结构的处理 [quick]
├── Task 6: 更新测试断言 [quick]
└── Task 7: 执行测试验证 [quick]
```

### Dependency Matrix

- **Task 1-4**: 无依赖，可并行
- **Task 5**: 依赖 Task 1-4
- **Task 6**: 依赖 Task 1-4
- **Task 7**: 依赖 Task 5, Task 6

### Agent Dispatch Summary

- **Wave 1**: 4 个 quick 任务，并行执行
- **Wave 2**: 3 个 quick 任务，顺序执行

---

## TODOs

- [ ] 1. 移除 handlers.rs:250 的 strip 调用

  **What to do**:
  - 找到 `forwarder::strip_downstream_reasoning_request(&mut body);` 这行
  - 注释掉或删除这行
  - 保留其他逻辑不变

  **Must NOT do**:
  - 不修改其他代码
  - 不删除函数定义

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 2, 3, 4)
  - **Blocks**: Task 5, Task 6, Task 7
  - **Blocked By**: None

  **References**:
  - `src-tauri/src/proxy/handlers.rs:250` - strip 调用位置

  **Acceptance Criteria**:
  - [ ] `forwarder::strip_downstream_reasoning_request(&mut body);` 已移除
  - [ ] `cargo build` 无错误

  **QA Scenarios**:

  ```
  Scenario: 编译验证
    Tool: Bash
    Steps:
      1. 执行 `cargo build`
      2. 检查退出码
    Expected Result: 退出码 0，无编译错误
    Evidence: .sisyphus/evidence/task-1-build.txt
  ```

  **Commit**: YES
  - Message: `fix: remove strip_downstream_reasoning_request from Chat Completions handler`
  - Files: `src-tauri/src/proxy/handlers.rs`
  - Pre-commit: `cargo build`

- [ ] 2. 移除 handlers.rs:327 的 strip 调用

  **What to do**:
  - 找到 `forwarder::strip_downstream_reasoning_request(&mut openai_body);` 这行
  - 注释掉或删除这行
  - 保留其他逻辑不变

  **Must NOT do**:
  - 不修改其他代码
  - 不删除函数定义

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 3, 4)
  - **Blocks**: Task 5, Task 6, Task 7
  - **Blocked By**: None

  **References**:
  - `src-tauri/src/proxy/handlers.rs:327` - strip 调用位置

  **Acceptance Criteria**:
  - [ ] `forwarder::strip_downstream_reasoning_request(&mut openai_body);` 已移除
  - [ ] `cargo build` 无错误

  **QA Scenarios**:

  ```
  Scenario: 编译验证
    Tool: Bash
    Steps:
      1. 执行 `cargo build`
      2. 检查退出码
    Expected Result: 退出码 0，无编译错误
    Evidence: .sisyphus/evidence/task-2-build.txt
  ```

  **Commit**: YES
  - Message: `fix: remove strip_downstream_reasoning_request from Claude handler`
  - Files: `src-tauri/src/proxy/handlers.rs`
  - Pre-commit: `cargo build`

- [ ] 3. 移除 responses_handler.rs:85 的 strip 调用

  **What to do**:
  - 找到 `forwarder::strip_downstream_reasoning_request(&mut chat_body);` 这行
  - 注释掉或删除这行
  - 保留 responses_handler.rs:87-96 的 Responses 专属字段剥离

  **Must NOT do**:
  - 不修改 responses_handler.rs:87-96 的代码
  - 不删除函数定义

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 2, 4)
  - **Blocks**: Task 5, Task 6, Task 7
  - **Blocked By**: None

  **References**:
  - `src-tauri/src/proxy/responses_handler.rs:85` - strip 调用位置
  - `src-tauri/src/proxy/responses_handler.rs:87-96` - Responses 专属字段剥离（保留）

  **Acceptance Criteria**:
  - [ ] `forwarder::strip_downstream_reasoning_request(&mut chat_body);` 已移除
  - [ ] responses_handler.rs:87-96 的代码保持不变
  - [ ] `cargo build` 无错误

  **QA Scenarios**:

  ```
  Scenario: 编译验证
    Tool: Bash
    Steps:
      1. 执行 `cargo build`
      2. 检查退出码
    Expected Result: 退出码 0，无编译错误
    Evidence: .sisyphus/evidence/task-3-build.txt
  ```

  **Commit**: YES
  - Message: `fix: remove strip_downstream_reasoning_request from Responses handler`
  - Files: `src-tauri/src/proxy/responses_handler.rs`
  - Pre-commit: `cargo build`

- [ ] 4. 移除 forwarder.rs:679-683 的 strip 调用

  **What to do**:
  - 找到 forwarder.rs:679-683 的条件 strip 逻辑
  - 注释掉或删除整个 if-else 块
  - 保留 normalize_reasoning_fields 逻辑（forwarder.rs:685-693）

  **Must NOT do**:
  - 不修改 normalize_reasoning_fields 函数
  - 不删除函数定义

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 2, 3)
  - **Blocks**: Task 5, Task 6, Task 7
  - **Blocked By**: None

  **References**:
  - `src-tauri/src/proxy/forwarder.rs:679-683` - strip 调用位置
  - `src-tauri/src/proxy/forwarder.rs:685-693` - normalize_reasoning_fields（保留）

  **Acceptance Criteria**:
  - [ ] forwarder.rs:679-683 的 strip 逻辑已移除
  - [ ] forwarder.rs:685-693 的 normalize_reasoning_fields 保持不变
  - [ ] `cargo build` 无错误

  **QA Scenarios**:

  ```
  Scenario: 编译验证
    Tool: Bash
    Steps:
      1. 执行 `cargo build`
      2. 检查退出码
    Expected Result: 退出码 0，无编译错误
    Evidence: .sisyphus/evidence/task-4-build.txt
  ```

  **Commit**: YES
  - Message: `fix: remove strip_downstream_reasoning_request from forwarder`
  - Files: `src-tauri/src/proxy/forwarder.rs`
  - Pre-commit: `cargo build`

- [ ] 5. 验证转换函数对格式专属结构的处理

  **What to do**:
  - 检查 claude_to_openai_request 函数对 thinking content blocks 的处理
  - 检查 responses_to_openai_chat_request 函数对 reasoning input items 的处理
  - 确认格式专属结构被正确转换

  **Must NOT do**:
  - 不修改转换函数逻辑

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 2 (after Tasks 1-4)
  - **Blocks**: Task 7
  - **Blocked By**: Tasks 1, 2, 3, 4

  **References**:
  - `src-tauri/src/proxy/handlers.rs` - claude_to_openai_request 函数
  - `src-tauri/src/proxy/protocol/responses.rs` - responses_to_openai_chat_request 函数

  **Acceptance Criteria**:
  - [ ] 确认 thinking content blocks 被正确转换
  - [ ] 确认 reasoning input items 被正确处理
  - [ ] 无格式专属结构泄漏到上游

  **QA Scenarios**:

  ```
  Scenario: 转换函数验证
    Tool: Bash
    Steps:
      1. 阅读 claude_to_openai_request 函数
      2. 阅读 responses_to_openai_chat_request 函数
      3. 确认格式专属结构被正确处理
    Expected Result: 格式专属结构被正确转换或剥离
    Evidence: .sisyphus/evidence/task-5-verify.txt
  ```

  **Commit**: NO

- [ ] 6. 更新测试断言

  **What to do**:
  - 找到 forwarder.rs 中的 strip 相关测试
  - 更新测试断言，反映新的行为（不剥离 reasoning）
  - 确保测试通过

  **Must NOT do**:
  - 不删除测试用例

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 2 (after Tasks 1-4)
  - **Blocks**: Task 7
  - **Blocked By**: Tasks 1, 2, 3, 4

  **References**:
  - `src-tauri/src/proxy/forwarder.rs` - strip 相关测试

  **Acceptance Criteria**:
  - [ ] 测试断言已更新
  - [ ] `cargo test` 无失败

  **QA Scenarios**:

  ```
  Scenario: 测试验证
    Tool: Bash
    Steps:
      1. 执行 `cargo test`
      2. 检查退出码
    Expected Result: 退出码 0，所有测试通过
    Evidence: .sisyphus/evidence/task-6-test.txt
  ```

  **Commit**: YES
  - Message: `test: update strip test assertions for reasoning passthrough`
  - Files: `src-tauri/src/proxy/forwarder.rs`
  - Pre-commit: `cargo test`

- [ ] 7. 执行测试验证

  **What to do**:
  - 启动 API Switch（本地 9090 端口）
  - 执行 12.10 节的 7 个测试用例
  - 记录测试结果

  **Must NOT do**:
  - 不修改代码

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 2 (after Tasks 5, 6)
  - **Blocks**: None
  - **Blocked By**: Tasks 5, 6

  **References**:
  - `docs/openai-reasoning-analysis.md:12.10` - 测试方案

  **Acceptance Criteria**:
  - [ ] T1-T7 测试用例全部通过
  - [ ] 测试结果记录到文档

  **QA Scenarios**:

  ```
  Scenario: T1 - Chat + reasoning_effort → 推理模型
    Tool: Bash (curl)
    Steps:
      1. 执行 curl 命令发送 Chat 请求（带 reasoning_effort）
      2. 检查响应状态码
      3. 检查响应中是否包含 reasoning_content
    Expected Result: Status 200，response 中包含 reasoning_content 字段
    Evidence: .sisyphus/evidence/task-7-t1.txt

  Scenario: T2 - Chat + reasoning_effort → 非推理模型
    Tool: Bash (curl)
    Steps:
      1. 执行 curl 命令发送 Chat 请求（带 reasoning_effort）到非推理模型
      2. 检查响应状态码
      3. 检查响应中是否包含 reasoning_content
    Expected Result: Status 200，无 reasoning_content（非推理模型忽略 reasoning_effort）
    Evidence: .sisyphus/evidence/task-7-t2.txt
  ```

  **Commit**: NO

---

## Final Verification Wave

- [ ] F1. **Plan Compliance Audit** — `oracle`
  验证所有修改是否符合分析文档第12节的方案。
  Output: `Must Have [N/N] | VERDICT: APPROVE/REJECT`

- [ ] F2. **Code Quality Review** — `unspecified-high`
  运行 `cargo test`，检查代码质量。
  Output: `Build [PASS/FAIL] | Tests [N pass/N fail] | VERDICT`

- [ ] F3. **Real Manual QA** — `unspecified-high`
  执行 T1-T7 测试用例，验证 reasoning 功能。
  Output: `Scenarios [N/N pass] | VERDICT`

---

## Commit Strategy

- **Task 1-4**: 每个任务单独提交
- **Task 6**: 单独提交
- **Task 7**: 不提交（测试结果记录到文档）

---

## Success Criteria

### Verification Commands

```bash
cargo build  # Expected: 编译成功
cargo test   # Expected: 所有测试通过
```

### Final Checklist

- [ ] 4 个位置的 strip 调用已移除
- [ ] responses_handler.rs:87-96 的 Responses 专属字段剥离保留
- [ ] 所有测试通过
- [ ] T1-T7 测试用例全部通过
