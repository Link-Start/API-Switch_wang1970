# API Switch

> 绿色便携的个人 AI 网关：多渠道路由、五协议转换、智能容错，一站管理所有 AI 服务。

API Switch 是一个本地优先的 AI API 管理与转发中心。它把 OpenAI-compatible、OpenAI Responses、Claude、Gemini、Azure OpenAI 等不同上游接入到同一个本地入口，面向桌面、Web Admin 和无头服务器场景提供统一管理、路由、日志和故障转移能力。

> 定位说明：API Switch 是个人本地工具，默认信任本机环境；如要暴露到公网，需要自行增加反向代理、TLS、访问控制等安全措施。

---

## 核心能力

| 能力 | 说明 |
|------|------|
| 统一代理入口 | 客户端只需连接一个本地 OpenAI-compatible 入口，即可访问多个上游渠道。 |
| 五协议适配 | 支持 OpenAI-compatible、OpenAI Responses、Claude Messages、Gemini、Azure OpenAI 的接入与转换。 |
| 模型池与分组路由 | 支持手动模型、模型别名、模型分组、精确匹配、模糊匹配和 AUTO 兜底。 |
| 智能排序 | 支持自定义顺序、最快优先、最新优先，并可结合测速与推荐分调整路由优先级。 |
| 三级容错 | DB 模型冷却、内存熔断器、渠道级冷冻逐层兜底，失败后自动尝试下一个可用条目。 |
| 空流保护 | HTTP 200 但流式响应长期无有效输出时视为失败，避免上游挂死后客户端一直等待。 |
| 使用日志与看板 | 记录请求、成功率、Token、模型分布、错误路径和敏感信息脱敏后的排障数据。 |
| Desktop / Web Admin 共用 UI | 同一套 React 页面同时服务 Tauri 桌面端和浏览器管理端。 |
| Headless 运行 | 支持无 GUI 场景，仅启动后端代理与管理服务，适合服务器、NAS 或远程主机。 |
| 连接应用 | 支持为 OpenCode CLI、Codex CLI、Claude Code、Zed 等工具生成或写入连接配置。 |

---

## 支持的协议与上游

| 类型 | 认证方式 | 典型路径 / 行为 |
|------|----------|----------------|
| OpenAI-compatible | Bearer Token | `/v1/chat/completions`、`/v1/models` |
| OpenAI Responses | Bearer Token | `/v1/responses` 兼容层，支持 text、function tools、streaming 和 Chat fallback |
| Claude Messages | `x-api-key` | Claude Messages 与 OpenAI Chat 主链路转换 |
| Gemini | query `key` / OpenAI 兼容路径 | 支持 Gemini OpenAI-compatible endpoint 与部分原生端点 |
| Azure OpenAI | `api-key` header | 通过 deployment 名参与路由和上游请求构造 |
| Custom | Bearer Token | 面向第三方 OpenAI-compatible 服务或中转站 |

说明：当前内部转换仍以 OpenAI Chat Completions 作为主要中间层。对 Responses、Claude、Gemini 的高级语义，项目会按目标协议白名单做边界收口，避免不支持字段误透传。

---

## 运行模式

### Desktop

Tauri v2 桌面应用，包含：

- React 管理界面
- 本地代理服务
- Tauri IPC 命令
- 系统托盘
- 本地窗口与设置管理

### Web Admin

Web Admin 使用同一套 React 页面，通过 HTTP Admin API 管理后端：

- 默认用于浏览器访问管理界面
- 使用 Bearer Token 登录与鉴权
- 与桌面端共享渠道、模型池、令牌、日志、设置等业务能力

### Headless / Server-only

无 GUI 环境可只启动后端能力：

```bash
api-switch --headless
# 或
API_SWITCH_HEADLESS=1 api-switch
```

适合服务器、NAS、远程主机或只需要代理服务的场景。

### Android / Mobile

Android 当前作为同一产品的移动构建壳与编译分支处理，不是单独产品线。目标是复用同一套核心逻辑和响应式 UI；真机代理监听、WebView 生命周期、loopback/cleartext 策略仍属于后续验证范围。

---

## 快速开始

### 下载运行

1. 到 [Releases](https://github.com/wang1970/API-Switch/releases) 下载对应平台构建。
2. 启动 API Switch。
3. 在 **渠道管理** 添加上游 API Base URL 与 API Key。
4. 拉取模型，或在 **API 管理** 手动添加模型。
5. 在客户端中把 Base URL 指向 API Switch 本地代理端口。

### 客户端配置

默认代理端口：`9090`。

```text
API Base URL: http://127.0.0.1:9090
API Key: 任意值；如开启强制 Access Key，则填写 API Switch 中创建的客户端令牌
Model: auto 或具体模型名 / 分组名
```

常见用法：

| 模型参数 | 行为 |
|----------|------|
| `auto` | 从 AUTO 组中选择已启用、未冷却的条目。 |
| 具体模型名 | 优先精确匹配，失败后按规则进入模糊匹配或 AUTO fallback。 |
| 分组名 | 可把多个上游模型归入同一逻辑组，让客户端使用稳定名称。 |

---

## 路由与容错

API Switch 的核心请求链路：

```text
Client / AI Tool
  -> API Switch Proxy Endpoint
  -> protocol parser / compatibility layer
  -> router / failover / cooldown
  -> upstream provider
  -> response converter / stream relay
  -> usage log / dashboard stats
```

容错机制：

- **L1 DB 冷却（模型级）**：失败模型写入 SQLite 冷却时间，重启后仍生效。
- **L2 内存熔断器（模型 + 渠道级）**：连续失败后短时间跳开，支持半开试探恢复。
- **L3 渠道冷冻（渠道级）**：账号额度、密钥或通道级故障时，整条渠道临时退出路由。
- **状态码禁用**：401 / 403 / 410 等不可恢复错误可将对应条目标记为禁用。
- **流式空响应检测**：SSE 长时间无有效数据时触发失败处理。
- **日志可追踪**：失败日志记录尝试路径，便于定位在哪个候选或协议转换阶段失败。

---

## 管理界面

主要页面：

- Dashboard：请求量、成功率、Token、模型分布、趋势图。
- 渠道管理：上游渠道、URL 探测、模型拉取、协议识别。
- API 管理：模型池、分组、排序、别名、启用状态。
- 令牌管理：客户端 Access Key。
- 使用日志：分页、筛选、展开详情、错误排查。
- 系统设置：端口、鉴权、冷却、主题、语言、Web Admin 等。
- 连接应用：为常见 AI 工具生成或写入配置。

---

## 本地开发

环境要求：

- Node.js / pnpm
- Rust 1.85+
- Tauri v2 依赖环境

常用命令：

```bash
pnpm install
pnpm dev              # 构建 Web Admin 后启动桌面开发模式
pnpm build            # 桌面构建
pnpm build:web-admin  # 构建 Web Admin 前端
pnpm typecheck        # TypeScript 类型检查
```

Android 相关命令保留为移动构建分支使用：

```bash
pnpm android:init
pnpm android:dev
pnpm android:build
```

---

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri v2 |
| 后端 | Rust、axum、reqwest |
| 数据库 | SQLite / rusqlite，WAL 模式 |
| 前端 | React 19、TypeScript、Vite |
| UI | Radix UI、Tailwind CSS v4 |
| 状态管理 | TanStack React Query |
| 图表 | Recharts |
| 国际化 | i18next / react-i18next |

---

## 数据与安全

- 数据默认存储在本地 SQLite 数据库中。
- 上游 API Key 与客户端 Access Key 分离。
- Web Admin 使用登录与 Bearer Token。
- 日志展示会对敏感信息做脱敏处理。
- 本项目不是公网多租户网关；公网使用需自行加固。

---

## 文档

- [中文使用指南](GUIDE_CN.md)
- [English Guide](GUIDE.md)
- [项目计划书](PLAN.md)
- [技术白皮书](WHITEPAPER.md)

---

## 项目状态

API Switch 当前面向个人本地使用持续迭代。后续重点包括：中立内部协议 IR、Capability Router、Gemini 原生端点补全、Responses / Claude / Gemini 高级语义保真、Web Admin 闭环优化等。

---

如果这个项目对你有帮助，欢迎在 [GitHub](https://github.com/wang1970/API-Switch) 点 Star。

## 💬 交流群

欢迎加入微信交流群，扫码添加：

![微信群](wx1.jpg)
