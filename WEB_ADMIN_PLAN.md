# Web Admin 管理端 — 桌面单源 / Web 动态翻译实施计划

> 版本: 5.0
> 更新日期: 2026-05-07
> 状态: 当前唯一执行基线

---

## 1. 目标

本轮开发只围绕以下目标进行：

- **Desktop UI 继续正常可用**，不破坏当前主使用路径
- **Web UI 不是独立手写产品线**，而是基于 Desktop UI 动态自动翻译生成的派生视图
- **Web 规范入口固定为 `9090/`**，不是 `/admin`
- **一个 binary 同时支持桌面环境与无桌面环境**
- **旧 Web 跑偏实现必须在后期被清理掉**，避免继续污染开发进程

核心定义：

```text
Desktop = 唯一主实现 / 真相源
Translation Layer = Desktop -> Web 的动态自动翻译层，可缓存
Web = 派生视图，不是第二套长期手写前端
```

---

## 2. 硬性约束

### 2.1 单源约束

- Desktop UI 是唯一主实现
- Web UI 不允许重新演化成独立真相源
- 新功能优先落在 Desktop 主实现，再决定是否导出到 Web

### 2.2 路径约束

- Web 唯一规范入口：`http://127.0.0.1:9090/`
- `/admin` 不再是目标路径
- 任何新设计不得再把 `/admin` 当作长期路径基线

### 2.3 翻译层约束

- 必须支持**动态自动翻译生成 Web UI**
- 必须允许缓存
- 缓存必须受页面结构版本 / 规则版本约束
- 必须允许局部降级，不能因局部翻译失败拖垮整个 Web UI
- 不能翻译的能力必须显式声明，而不是临时手写绕过

推荐规则示例：

```text
visibleInWeb: false
webMode: hidden | readonly | full
requiresNative: true
cacheable: true
```

### 2.4 业务逻辑约束

- `Service` 是唯一业务逻辑来源
- `commands/*` 与 Web handler 只做协议适配
- 禁止在不同入口分别生长两套业务逻辑

### 2.5 API / 数据流约束

前端不得直接耦合运行环境：

```ts
invoke("...")
fetch("...")
```

必须通过统一边界访问：

```ts
const api = useApiAdapter();
api.xxx.yyy();
```

### 2.6 文档约束

- 本文件只保留**当前执行基线**
- 不再保留偏离方向、旧路线、混杂历史描述
- 后续如需记录历史，只能单独写历史文档，不得反向污染主计划

---

## 3. 当前判断

### 3.1 当前稳定基线

- Desktop 是当前唯一稳定主路径
- 现有项目可继续以 Desktop 为主正常使用

### 3.2 当前策略

当前开发策略固定为：

```text
P1：先保证正常使用桌面
→ 在不破坏 Desktop 主路径的前提下，推进新的 Web 生成路径
→ 先建 Translation Layer，能翻多少翻多少
→ 再把 Web 稳定收口到 9090/
→ 最后清理旧 Web 跑偏实现
```

说明：

- P1 的第一目标不是先完成 Web，而是先保证 Desktop 持续正常使用
- 只要某项工作会影响 Desktop 当前主路径，就不属于 P1 可接受方案
- 在你的当前判断下，不预设必须修改 Desktop UI；只有真实开发证明存在阻塞时，才另行回写计划

### 3.3 明确认定为“跑偏内容”的方向

以下方向已经确认偏离，不得再作为开发目标：

- 把 Web 当成独立手写前端长期维护
- 把旧 `src/web-admin/*` 当成长期主实现继续扩展
- 把 `/admin` 当前缀路径当成目标架构
- 把“共享 feature + 独立 Web 壳长期共存”当成最终目标
- 先补旧 Web，再慢慢收口

---

## 4. 执行阶段

### Phase 1：先保证正常使用桌面（P1）

目标：P1 的第一优先级是保证 Desktop 持续正常使用。后续所有工作都不能破坏当前 Desktop 主使用路径。

必须保证：

1. Tray 行为不回退
2. Channels 正常可用
3. Settings 正常可用
4. Proxy 启停正常可用
5. 设置修改后的联动行为不回退

阶段完成标准：

```text
Desktop 仍然是稳定主用入口；
后续 Translation Layer / Web 改动都不能破坏它。
```

---

### Phase 2：先建立 Translation Layer，能翻多少翻多少

目标：先建立可工作的动态翻译层，而不是继续补旧 Web 页面。

优先完成：

1. 定义 Desktop UI 的可翻译结构 / 规则 / 元数据边界
2. 建立 Desktop -> Web 的动态自动翻译机制
3. 建立缓存与失效规则
4. 建立局部降级机制
5. 优先覆盖最容易翻译、最有价值的模块：
   - Settings
   - Channels
   - Tokens
   - Logs / Dashboard 中结构化、只读优先的部分

阶段完成标准：

```text
Desktop 保持可用；
Web 已能由 Translation Layer 动态生成第一批可用页面；
即使覆盖率未满，也已脱离“旧手写 Web 主导”路线。
```

---

### Phase 3：根据 Web 需求反向调整 Desktop UI

目标：不是重写 Desktop，而是只在确实妨碍翻译时，为提升可翻译性调整 Desktop。

优先完成：

1. 识别 Desktop 中妨碍翻译的结构
2. 把桌面独有能力显式标记为不导出给 Web
3. 统一字段语义、页面结构、可见性规则
4. 扩大 Translation Layer 可覆盖的页面范围

阶段完成标准：

```text
大多数核心桌面管理能力都能稳定派生 Web 视图；
不能翻译的部分已被显式声明，而不是靠临时分叉处理。
```

---

### Phase 4：将 Web 收口到 `9090/`

目标：让 Web 的实际访问模型与计划目标一致。

优先完成：

1. `9090/` 成为唯一规范 Web 入口
2. Web 路径模型与 Translation Layer 输出一致
3. 静态资源、登录、页面路由、API 共存关系理顺
4. 代理 API 与 Web UI 在同一端口稳定共存

阶段完成标准：

```text
Web 的稳定访问入口为 9090/；
不存在把 /admin 当成长期目标路径的残留设计。
```

---

### Phase 5：清理旧 Web 跑偏实现

前置条件：

- Desktop 继续稳定可用
- Translation Layer 已能稳定生成可用 Web UI
- Web 已覆盖当前核心需求
- 新路径已验证可替代旧实现

清理目标：

1. 废除旧 Web 独立实现作为主路径
2. 删除与旧路线绑定的过渡结构
3. 只保留真正有复用价值的资产
4. 统一代码语义：Desktop 是主实现，Web 是派生视图

清理对象包括但不限于：

- 旧 `src/web-admin/*` 中仅服务于旧路线的独立壳 / bootstrap / API glue
- `/admin` 作为长期主入口的路径假设
- 旧双入口、双真相源相关的过渡逻辑
- 仅为了旧 Web 方案存在的 build / route / auth glue

阶段完成标准：

```text
旧 Web 跑偏实现不再是默认开发基线；
开发进程不再被旧路线继续污染。
```

---

### Phase 6：工程化封口

优先完成：

1. 防分叉规则
2. CI / review 规则
3. 自动化一致性回归
4. Translation Layer 稳定性与缓存验证
5. 禁止重新长出独立手写 Web 业务页的检查规则

---

### Phase 7：CLI 后置评估

只有在以下前提都满足时才评估：

- 单一 binary 仍然成立
- Desktop + Web 已稳定
- Standalone 已成立
- 不破坏 Desktop 主体验

---

## 5. 联动一致性检查规则

每次宣称“Desktop / Web 一致”时，必须同时完成：

### 5.1 静态层

- 类型定义一致
- 默认值一致
- UI 选项一致
- Translation Layer 导出的结构与 Web 实际渲染一致

### 5.2 数据流层

必须端到端追踪：

- Desktop 真实触发行为
- Translation Layer 导出 / 缓存
- Adapter / IPC / HTTP 序列化
- Rust 反序列化
- DB 写入逻辑
- Web 重新读取并渲染

### 5.3 风险层

以下全部视为高风险信号：

- `Partial<AppSettings>`
- `as any`
- 临时手写 Web 分叉逻辑
- 无规则描述的“这个先不翻译”

只要任一链路无法确认，就不能给出“一致”结论。

---

## 6. 本次完成定义（DoD）

满足以下全部条件，才算完成本轮目标：

1. 一个 binary 同时支持桌面环境与无桌面环境运行
2. 有 GUI 环境下自动进入 Combined
3. 无 GUI 环境下自动进入 Standalone
4. Desktop 继续保持当前主路径可用
5. Translation Layer 能动态自动翻译生成可用 Web UI
6. Translation Layer 支持缓存，并具备清晰失效规则
7. Web 的规范入口为 `9090/`
8. Web 不再是第二套长期手写业务前端
9. 旧 Web 跑偏实现已进入可清理状态，并最终完成清理
10. Settings 等关键链路完成完整对象更新一致性验证

---

## 7. 当前唯一推荐执行顺序

```text
Phase 1：锁桌面主路径
Phase 2：先建立 Translation Layer，能翻多少翻多少
Phase 3：根据 Web 需求反向调整 Desktop UI
Phase 4：将 Web 收口到 9090/
Phase 5：清理旧 Web 跑偏实现
Phase 6：工程化封口
Phase 7：CLI 后置评估
```
