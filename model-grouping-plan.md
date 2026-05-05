# 模型分组（Model Grouping）实施开发计划

> 日期: 2026-05-05
> 当前主目标: **一次性完成桌面端最小闭环**
> 说明: 当前最小实现不是只做页面分组，也不是只做 tray，而是必须把 **API 管理分组 Tabs -> 设置中的默认分组 -> 转发模型选择 -> 托盘联动** 一次性做通。除此之外的内容全部后置。

---

## 0. 本轮目标

本轮只做 **P0 桌面端最小闭环**，必须一次性打通：

1. **API 管理分组 Tabs**
2. **设置中的默认分组**
3. **默认分组参与转发模型选择**
4. **Tray 联动**

唯一状态源锁定为：

```text
settings / config 中的 active_group 字段
```

任何页面本地 state、tray 本地 state、转发逻辑临时 state，都不能成为第二份真相源。

---

## 1. 范围与边界

### 1.1 本轮必须完成

```text
API 管理选择 Tabs
  -> 设置中的默认分组
  -> 转发时的模型选择
  -> 托盘联动
```

这四者缺一不可。只有一起成立，才算桌面端最小实现完成。

### 1.2 本轮不接受的“伪完成”

以下都视为未完成：

- 只做页面 Tabs，不改设置默认项
- 只做数据库字段，不改桌面转发逻辑
- 只做 tray 展示，不改页面与默认分组
- 只做局部分组选择，不打通转发模型选择

### 1.3 明确后置项

P0 完成前，不处理：

- Web 端跟进
- 与桌面主链无关的新功能扩展

---

## 2. 统一语义（锁定）

### 2.1 分组基础语义

| 项目 | 锁定语义 |
|------|---------|
| 默认分组 | `auto` |
| `auto` 含义 | 默认分组，不是 `ALL` |
| `ALL` | 不存在 |
| 页面 Tabs | **Tabs = 分组**，有几个分组就显示几个 Tabs |
| 新增 entry 默认值 | `group_name = 'auto'` |

### 2.2 设置项语义

设置页中的旧“默认排序”语义，本轮必须改造成：

```text
默认分组
```

要求：

- 设置页展示当前真实存在的分组列表
- 当前选中值 = 当前默认分组
- 默认值 = `auto`
- 该状态源在当前实现中对应 `active_group`
- 禁止继续把 `custom / latest / fastest` 当作当前主配置语义

### 2.3 默认转发语义

默认转发必须按以下规则工作：

```text
先按当前默认分组取模型池
-> 再按该分组内优先级顺序选择可用 entry
```

同时保留：

```text
分组精确匹配 > 模型名子串匹配（长度 ≥ 5）> 默认分组兜底
```

这里的“默认分组兜底”必须来自 settings/config 当前状态源，而不能只停留在计划语义或硬编码回 `auto`。

### 2.4 Tray 联动语义

Tray 必须与上述语义完全一致：

- tray 显示当前默认分组
- tray 切分组，本质是在切默认分组
- tray 显示的是当前默认分组下的模型
- tray 切模型，本质是在改该分组内优先级
- tray 切分组后，页面与转发逻辑都必须跟随

---

## 3. 当前状态判断（按当前实现更新）

根据当前代码、构建验证与单测结果，四段链路状态如下：

| 链条 | 当前状态 | 说明 |
|------|----------|------|
| API 管理选择 Tabs | ✅ 已完成 | 页面顶部 Tabs 已只保留真实分组，`ALL` 已去除，`auto` 作为真实分组；点击 Tabs 直接写回 `settings.active_group` |
| 设置中的默认排序 -> 默认分组 | ✅ 已完成 | 设置页主语义已切换为默认分组，候选值来自真实分组列表，绑定到 `active_group` |
| 默认分组参与转发模型选择 | ✅ 已完成 | 默认请求 `auto` 已先按 `active_group` 选择分组池，再按组内 `sort_index` 选择可用 entry |
| 托盘联动 | ✅ 已完成 | tray 已读写 `active_group`，页面与转发都能感知更新；tray 当前模型顺序已与 API 管理 / 转发统一为当前分组内 `sort_index` |

当前结论：

```text
P0 四段链路从代码、构建和单测层面已打通。
当前剩余工作是桌面实际运行联调与人工验收，而不是继续扩展无关功能。
```

---

## 4. 实施原则

### 4.1 必须遵守

- 不先碰 Web，只保桌面主链
- 不做增强项
- 不引入第二份默认分组状态
- 不为了联动去破坏桌面 API 管理页稳定性
- 每一步都要有验证点，失败立即停在当前步修复

### 4.2 固定开发顺序

```text
Step A: 先保 API 管理页稳定 + 分组可维护
Step B: 再把设置页默认排序改造成默认分组
Step C: 再让默认分组接入转发逻辑
Step D: 最后接 Tray 和 settings 双向联动
Step E: 做四段链路联调验收
```

---

## 5. Phase A：桌面 API 管理页稳定化 + 分组维护

### 目标

确保桌面端 `API 管理` 页面稳定，并且真实按分组工作。

### 任务 A1：确认桌面主路径只走共享 `PoolManager`

检查：

- `src/pages/ApiPoolPage.tsx`
- `src/features/pool/PoolManager.tsx`

要求：

- `ApiPoolPage.tsx` 只保留薄壳职责
- 桌面和 Web 共用同一套 `PoolManager`
- 不保留桌面专属旧 API 池渲染逻辑

### 任务 A2：确认分组 Tabs 语义正确

要求：

- Tabs = 真实分组
- 没有 `ALL`
- `auto` 是真实分组，不是“全部”
- 空分组不显示
- 页面点击 Tabs 本质上是在修改 `settings.active_group`

### 任务 A3：确认 entry 分组可编辑

要求：

- entry 卡片能修改 `group_name`
- 可输入新分组名
- 修改后立即刷新
- 不白屏、不崩溃

### 任务 A4：确认新增 entry 默认进入 `auto`

检查三条路径：

1. 手动新增模型
2. `selectModels`
3. 历史数据升级

### A 阶段完成标准

- 桌面启动正常
- 进入 API 管理页不白屏
- 分组 Tabs 可切换
- entry 分组可改
- 新增 entry 默认进 `auto`

---

## 6. Phase B：设置页把“默认排序”改造成“默认分组”

### 目标

设置页正式成为 **默认分组唯一配置入口**。

### 任务 B1：找出旧字段和旧 UI

检查：

- `src/features/settings/SettingsEditor.tsx`
- `src/pages/SettingsPage.tsx`
- Rust settings 结构体 / config 读写路径
- 所有 `default_sort_mode`、`sort_mode`、`custom/latest/fastest` 的旧语义使用点

### 任务 B2：统一状态字段

当前实现采用：

```text
active_group: String
```

要求：

- 默认值：`auto`
- 老库升级后自动补齐
- 配置可读写
- 前后端字段名一致
- 不再新增第二个 `default_group` 字段

### 任务 B3：设置页 UI 改造

要求：

- “默认排序”改成“默认分组”
- 候选值来自当前真实分组列表
- 默认值为 `auto`
- 保存时写入统一 settings/config（当前实现字段为 `active_group`）

### 任务 B4：联动读取真实分组列表

设置页不能写死候选值，必须动态读取：

- 当前所有真实分组
- 没有组时至少保留 `auto`

### B 阶段完成标准

- 设置页不再以 `custom/latest/fastest` 作为主配置语义
- 设置页显示真实分组列表
- 设置可切换默认分组
- 保存后能持久化并重新读出

---

## 7. Phase C：默认分组接入默认转发

### 目标

默认转发真正先按 settings 里的默认分组选择模型。

### 任务 C1：梳理当前转发入口

检查：

- `src-tauri/src/proxy/router.rs`
- `src-tauri/src/proxy/server.rs`
- 任何读取 settings 的转发入口
- 当前 `resolve()` 的调用链

### 任务 C2：把“默认分组”接入默认请求路径

默认模式下，流程必须变成：

1. 读取 `settings.active_group`
2. 从该 group 的 enabled entries 中选
3. 按优先级取可用 entry
4. 若该组无可用项，再走既定兜底

### 任务 C3：保留已有匹配规则

仍保留：

- 分组精确匹配
- 模型名子串匹配（长度 ≥ 5）
- 默认分组兜底

关键要求：

- 这里的“默认分组兜底”必须来自 settings/config 当前状态源
- 模型选择顺序统一按 `sort_index`
- 不能无条件硬编码回 `auto`

### C 阶段完成标准

- 修改设置中的默认分组后
- 默认转发行为确实跟着变化
- 当前分组内模型选择顺序按 `sort_index`
- 精确分组匹配 / 子串匹配逻辑不被破坏

---

## 8. Phase D：Tray 接入默认分组唯一状态源

### 目标

tray 不再维护独立 group 真相，而是 settings/`active_group` 的桌面快捷入口。

### 任务 D1：审查 tray 当前状态

检查：

- `src-tauri/src/lib.rs`
- tray menu 构建逻辑
- tray 点击事件处理
- 当前 `active_group` 持久化位置

### 任务 D2：去除 tray 独立状态源

要求：

- tray 展示当前默认分组 = `settings.active_group`
- tray 切分组 = 写回 `settings.active_group`
- 不允许 tray 单独保存另一份“当前组”

### 任务 D3：tray 模型列表跟随默认分组

要求：

- tray 显示的是当前默认分组下的模型列表
- tray 切模型时，修改的是该组内优先级
- 切组后模型列表刷新
- tray 模型顺序与 API 管理 / 默认转发统一按 `sort_index`

### 任务 D4：设置页与 tray 双向同步

要求：

- Settings 改默认分组 -> tray 立即同步
- tray 改默认分组 -> settings 重新读取时一致
- 页面 / tray / 转发三方一致

### D 阶段完成标准

- tray 当前显示和 `settings.active_group` 一致
- tray 切组后 settings 同步
- settings 改组后 tray 同步
- tray 模型列表与当前默认分组一致
- tray 当前模型与 API 管理 / 默认转发一致

---

## 9. Phase E：数据库兼容与关键链路一致性检查

### 目标

确认不是“表面能跑”，而是所有关键读写路径真的一致。

### 任务 E1：Schema 检查

确认：

- `api_entries.group_name TEXT NOT NULL DEFAULT 'auto'`
- 启动迁移已接入
- 历史数据自动补为 `auto`

### 任务 E2：settings 配置兼容检查

确认：

- `active_group` 字段存在
- 老数据升级后可读写
- 默认值正确

### 任务 E3：查询链一致性检查

逐条确认：

- 页面读取 entry 能拿到 `group_name`
- tray 构建能拿到 `group_name`
- 转发 resolve 能拿到 `group_name`
- 所有 `row_to_entry()` 或等效转换链已补齐

### 任务 E4：新增路径一致性检查

确认以下都写入 `auto`：

1. 手动新增
2. `selectModels`
3. 旧库升级

---

## 10. 端到端验收用例

必须至少通过下面 8 条：

1. 在 API 管理中把某个模型改到 `coding`
2. 设置中把默认分组改成 `coding`
3. 发起默认转发，确认从 `coding` 组取模型
4. tray 同步显示 `coding`
5. tray 切回 `auto`
6. 设置中的默认分组同步变回 `auto`
7. 默认转发恢复从 `auto` 组取模型
8. 页面 Tabs / 设置 / tray / 实际转发结果一致

---

## 11. 失败判定

出现任一情况都算 **P0 未完成**：

- 设置页还在用 `custom/latest/fastest` 旧语义
- 改默认分组后，默认转发行为没变化
- tray 切组后 settings 不同步
- 页面 / settings / tray / 转发四者不一致
- 为联动导致桌面 API 管理页重新白屏或失稳

---

## 12. 执行顺序（落地版）

### 第一轮：只做断点确认

1. 查 settings 当前字段与旧语义引用点
2. 查 tray 当前 `active_group` 是否独立持久化
3. 查默认转发是否真的先读 `settings.active_group`
4. 输出断点清单

### 第二轮：先改 settings 成为唯一状态源

1. 配置结构改造
2. 设置页 UI 改造
3. 读写联调

### 第三轮：改转发逻辑接 `settings.active_group`

1. 接入默认分组
2. 保留现有匹配规则
3. 做请求验证

### 第四轮：tray 改为读写 `settings.active_group`

1. 去独立状态
2. 接 group 列表与模型列表刷新
3. 双向联动验证

### 第五轮：桌面端联调与构建

1. `cargo check`
2. `pnpm typecheck`
3. `pnpm build:renderer`
4. `cargo test router::tests`
5. 桌面手工冒烟

---

## 13. P0 验收清单

以下全部通过，才算当前最小实现完成。

### 13.1 桌面可运行
- [ ] 桌面能正常启动（待桌面人工确认）
- [ ] 左侧点击“API 管理”不白屏（待桌面人工确认）
- [ ] API 池页面不再出现“加载后全白”（待桌面人工确认）

### 13.2 分组 Tabs
- [x] 页面按真实分组显示 Tabs
- [x] 没有 `ALL`
- [x] `auto` 只表示默认分组，不表示全部
- [x] 切换 Tabs 能正常过滤列表
- [x] 点击 Tabs 会直接联动 `settings.active_group`

### 13.3 分组编辑
- [x] entry 卡片可修改分组
- [x] 输入新分组名后可生效
- [x] 修改后列表刷新正常
- [x] 修改分组不会让页面崩溃

### 13.4 设置默认分组
- [x] 设置页中的“默认排序”已改成“默认分组”语义
- [x] 设置页展示真实分组列表
- [x] 设置页可切换默认分组
- [x] 修改后可持久化保存

### 13.5 转发模型选择
- [x] 默认转发按当前默认分组取模型
- [x] 组内按优先级选择可用 entry
- [x] 修改默认分组后，默认转发行为跟着变化

### 13.6 托盘联动
- [x] tray 显示当前默认分组
- [x] tray 切分组后 settings 同步更新
- [x] settings 改默认分组后 tray 同步显示
- [x] tray 中模型列表与当前默认分组一致
- [x] tray 切模型后改的是该组内优先级

### 13.7 数据库兼容
- [x] schema 迁移已接入启动流程
- [x] 老库升级后 `group_name` 默认归入 `auto`
- [x] 默认分组设置项升级后存在且可用
- [x] 所有关键读写路径一致

### 13.8 构建检查
- [x] `cargo check` 通过
- [x] `pnpm typecheck` 通过
- [x] `pnpm build:renderer` 通过
- [x] `cargo test router::tests` 通过

---

## 14. 后续策略

当前主链已经完整，后续不预设大而空的增强路线。

原则：

- 先做桌面实际运行联调
- 真遇到明确痛点，再单独立项
- 不继续堆砌与主链无关的“体验增强”

---

## 15. 最终完成定义

这轮完成的标志不是“页面上看到了分组”，而是：

1. API 管理页能维护真实分组
2. 设置页能切换默认分组
3. 默认转发确实按默认分组工作
4. tray 显示并切换的是同一份默认分组
5. 四者共用同一个 settings/config 状态源（当前实现字段为 `active_group`）
6. 修改默认分组后，页面 / 转发 / tray 三方结果一致
7. 当前模型排序规则在 API 管理 / 转发 / 托盘 三者之间一致（当前实现统一为 `sort_index`）
