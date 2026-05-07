# Web Admin 管理端 — 单一 UI / 双运行时实施计划

> 版本: 6.0
> 更新日期: 2026-05-07
> 状态: 当前唯一执行基线

---

## 1. 核心结论

本项目 Web Admin 的正确方向是：

```text
一套 React UI
+ 一套页面/组件/状态管理
+ 一个 ApiAdapter 边界
+ 两种运行时调用方式
```

不是：

```text
Desktop UI -> Translation Layer -> Web UI
```

也不是：

```text
Desktop 一套页面 + Web 手写另一套页面
```

当前重新确认后的架构定义：

```text
UI Source = src/App.tsx + src/pages/* + src/features/*
Desktop Runtime = 同一套 UI + tauriApiAdapter + Tauri invoke
Web Runtime = 同一套 UI + webAdminApiAdapter + HTTP fetch
Service = 唯一业务逻辑来源
Rust commands/admin handlers = 协议适配层
```

**禁止再引入 Translation Layer、Schema 翻译层、动态 UI 翻译缓存、Desktop -> Web 自动翻译中间层。**

---

## 2. 目标

本轮目标只围绕以下事项：

1. Desktop UI 继续正常可用，不破坏当前主使用路径。
2. Web UI 使用和 Desktop 完全相同的 React 页面与组件。
3. Web 不再维护第二套长期手写业务页面。
4. 一个 binary 同时支持：
   - 有 GUI 环境：Desktop + Web Admin
   - 无 GUI 环境：Standalone Web Admin
5. Web 入口最终收口到根路径：

```text
http://127.0.0.1:9090/
```

开发环境中端口可能是 `9099/` 等配置值，但路径模型必须是根路径 `/`，不是 `/admin`。

---

## 3. 硬性约束

### 3.1 单一 UI 约束

- `src/` 是唯一 UI 真相源。
- Desktop 和 Web 都必须使用同一套：
  - `src/App.tsx`
  - `src/pages/*`
  - `src/features/*`
  - `src/components/*`
- 新功能只允许改这一套 UI。
- 禁止为了 Web 再手写一套业务页面。
- 禁止重新扩展旧 `src/web-admin/src/WebAdminApp.tsx` 成为第二套壳。

### 3.2 Adapter 边界约束

前端页面不得直接耦合运行环境：

```ts
invoke("...")
fetch("...")
```

页面必须通过统一边界访问：

```ts
const api = useApiAdapter();
api.xxx.yyy();
```

运行时选择规则：

```ts
isTauriRuntime() ? tauriApiAdapter : webAdminApiAdapter
```

### 3.3 业务逻辑约束

- `Service` 是唯一业务逻辑来源。
- `commands/*` 只负责 Tauri invoke 协议适配。
- `admin/*_handlers.rs` 只负责 HTTP 协议适配。
- 禁止在 commands 和 Web handlers 中分别长出两套业务逻辑。

### 3.4 路径约束

最终目标：

```text
Web UI: /
Web assets: /assets/*, /logo/*, /favicon.ico, /star.jpg
Proxy API: /v1/*, /v1beta/*, /openai/*
Admin API: 当前可保留 /admin/*，后续再评估是否迁移
```

当前优先级：

1. 先让 `/` 能显示同一套 UI。
2. 再处理登录、token、HTTP API 接线。
3. 最后处理 `/admin/*` API prefix 是否迁移。

不要为了 prefix 迁移阻塞“一套 UI 能呈现”。

### 3.5 禁止事项

以下方向已确认跑偏，禁止作为后续目标：

- Desktop -> Web Translation Layer
- UI schema 翻译层
- 动态自动翻译生成 Web UI
- Translation cache / rule version / page structure version
- 为 Web 独立维护一套业务页面
- 把旧 `src/web-admin/*` 当成长期产品线继续扩展
- 把 `/admin` 当成 Web UI 长期入口

---

## 4. 当前实现状态

### 4.1 已完成：同一套 UI 的 Web 构建验证

已完成一次可运行验证：

```text
src/App.tsx
  -> vite.config.web.ts
  -> dist-web-admin/
  -> Rust admin static_files.rs
  -> http://127.0.0.1:9099/
  -> 浏览器成功呈现 UI
```

验证结论：

```text
同一套 src/ React UI 可以作为 Web Admin 页面呈现。
```

这证明“单一 UI + adapter”路线可行。

### 4.2 本次已落地改动

#### `src/App.tsx`

已调整为运行时无关入口：

- 不再静态导入 Tauri API。
- 使用 `useApiAdapter()` 获取数据。
- Desktop 环境：
  - `tauriApiAdapter`
  - Tauri invoke
  - Tauri opener / version 动态 import
- Web 环境：
  - `webAdminApiAdapter`
  - HTTP fetch
  - `window.open`
- `WelcomeGuide` 等 Desktop-only 行为通过 `isTauriRuntime()` 隔离。

#### `vite.config.web.ts`

新增 Web 构建入口：

```text
src/ -> dist-web-admin/
```

构建 Web Admin 时不再使用旧 `src/web-admin/src/WebAdminApp.tsx` 作为主 UI。

#### `src/stubs/*`

新增 Web 构建用 Tauri stub：

- `src/stubs/tauri-core.ts`
- `src/stubs/tauri-app.ts`
- `src/stubs/tauri-opener.ts`

目的：让 Web 构建阶段不被 Tauri 模块阻塞。

#### `src-tauri/src/admin/static_files.rs`

修复根路径静态资源映射：

```text
/assets/index.js -> dist-web-admin/assets/index.js
/logo/x.svg      -> dist-web-admin/logo/x.svg
```

此前 `/` 能返回 HTML，但 `/assets/*.js` / `/assets/*.css` 404，原因是 `/assets/` 前缀被错误剥掉。

修复后，Web Admin 根路径 UI 能正常加载资源并呈现。

### 4.3 当前验证结果

开发环境实际验证：

```text
9098 = proxy port
  /health    200
  /v1/models 200
  /          404（正常，它不是 Web Admin 端口）

9099 = Web Admin port
  /              200
  /admin/version 200
  /assets/*.js   修复后应为 200
  UI             已成功呈现
```

---

## 5. 当前剩余工作

### Phase 1：稳住单一 UI 路线

目标：确保以后只改一套 UI。

任务：

1. 保留 `src/App.tsx` 作为 Desktop/Web 共同入口。
2. Web Admin 构建继续使用 `vite.config.web.ts`。
3. 禁止继续扩展旧 `src/web-admin/src/WebAdminApp.tsx`。
4. 检查页面里直接使用 Tauri API 的地方，逐步移动到 adapter 或 runtime guard 后面。
5. Desktop 回归：确保 Tauri 窗口仍正常。

完成标准：

```text
改一个页面，Desktop 和 Web 同时变化。
```

### Phase 2：接上 Web 功能

目标：让 Web UI 不只是能显示，还能通过 HTTP 正常读写数据。

任务：

1. 明确 Web Admin 登录/token 方案。
2. `webAdminApiAdapter` 保持统一 token 注入。
3. Settings / Channels / Pool / Tokens / Logs / Dashboard 逐页验证 HTTP 调用。
4. 对 Desktop-only 能力做显式降级：
   - tray
   - native event
   - native opener
   - 系统级能力
5. 不新增页面分叉，只在 adapter 或 runtime guard 处理差异。

完成标准：

```text
Web 上核心页面能使用同一套 UI 完成基本操作。
```

### Phase 3：清理旧 Web 壳

前置条件：

- `src/App.tsx` 在 Web Admin 中稳定呈现。
- 核心页面通过 HTTP adapter 可用。
- 登录/token 方案稳定。

清理对象：

```text
src/web-admin/src/WebAdminApp.tsx
src/web-admin/src/main.tsx
src/web-admin/src/api.ts 中只服务旧壳的部分
src/web-admin/vite.config.ts
src/web-admin/index.html
```

注意：

- 可复用的 HTTP 登录/token 辅助逻辑可以迁移到 `src/lib/`。
- 不能直接删除仍被构建或运行依赖的文件。

完成标准：

```text
旧 Web 壳不再参与主构建，不再作为开发基线。
```

### Phase 4：路径收口

目标：稳定 Web 入口模型。

优先级：

1. Web UI 根路径 `/` 稳定。
2. 静态资源 `/assets/*` 稳定。
3. Proxy API `/v1/*` 不受影响。
4. Admin API prefix 后续再按风险迁移。

完成标准：

```text
Web UI 的稳定访问入口为 /；
不再依赖 /admin 作为 UI 入口。
```

### Phase 5：工程化防回退

目标：防止再次长出第二套 UI 或翻译层。

任务：

1. CI / review 检查：禁止新增 Web 专属业务页面。
2. 检查页面直接 import Tauri API。
3. 检查页面直接 fetch HTTP。
4. 检查是否出现 Translation Layer / schema translator 等命名。
5. 建立 Desktop + Web 最小构建验证。

完成标准：

```text
代码层面阻止路线再次跑偏。
```

---

## 6. 联动一致性检查规则

每次宣称 Desktop / Web 一致，必须同时检查：

### 6.1 静态层

- 是否使用同一套页面组件。
- 类型定义是否一致。
- UI 选项是否一致。
- 默认值是否一致。

### 6.2 数据流层

必须端到端追踪：

```text
页面组件
-> useApiAdapter()
-> tauriApiAdapter / webAdminApiAdapter
-> Tauri invoke / HTTP request
-> Rust command / admin handler
-> Service
-> DB
-> 重新读取并渲染
```

### 6.3 风险层

以下全部视为高风险信号：

- `as any`
- `Partial<AppSettings>`
- 页面里直接 `invoke(...)`
- 页面里直接 `fetch(...)`
- `src/web-admin` 新增业务页面
- Translation Layer / schema translator / dynamic UI translator
- 为 Web 单独复制 Desktop 页面逻辑

只要任一链路无法确认，就不能说“一致”。

---

## 7. 本轮 DoD

满足以下条件才算本轮完成：

1. Desktop 继续保持当前主路径可用。
2. Web Admin 使用 `src/App.tsx` 同一套 UI 呈现。
3. Web Admin 根路径 `/` 能打开 UI。
4. 静态资源 `/assets/*` 能正常加载。
5. 核心页面通过 `useApiAdapter()` 访问数据。
6. 不再建设 Translation Layer。
7. 旧 `src/web-admin` 壳进入可清理状态。
8. Settings 等关键链路完成完整对象更新一致性验证。

---

## 8. 当前唯一推荐执行顺序

```text
Phase 1：稳住单一 UI 路线
Phase 2：接上 Web HTTP 功能
Phase 3：清理旧 Web 壳
Phase 4：路径收口
Phase 5：工程化防回退
CLI：后置评估
```

---

## 9. 执行记录

### 2026-05-07：路线修正

结论：今天 P1-P3 的 Translation Layer 方向已确认跑偏，后续不再沿用。

修正后的方向：

```text
同一个 UI，不做翻译层。
```

### 2026-05-07：单一 UI Web 呈现验证

已完成：

1. `src/App.tsx` 改为 Desktop/Web 共用入口。
2. 新增 `vite.config.web.ts`，将 `src/` 构建到 `dist-web-admin/`。
3. 新增 Tauri stub，保证 Web 构建可通过。
4. 修复 Rust static_files 根路径 asset 映射。
5. 开发环境 `9099/` 已能呈现同一套 UI。

当前结论：

```text
同一套 UI 路线已跑通第一步。
下一步只需要接登录/token 和 HTTP adapter 的功能细节。
```
