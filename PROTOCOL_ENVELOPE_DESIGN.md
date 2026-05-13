# 多协议翻译中间层设计

## 目标

API Switch 的协议翻译层需要同时支持 OpenAI Chat Completions、OpenAI Responses、Claude Messages、Gemini GenerateContent、Azure OpenAI 等主流协议，并保留项目自身位于中间层的路由、策略、审计、回退、协议补偿等特色能力。

本设计的核心目标是：

- 入口识别并标注来源协议。
- 出口根据目标协议做定向翻译和裁剪。
- 不建立庞大的万能中间协议。
- 不把 OpenAI Chat 作为唯一内部标准，避免过早丢失 Responses、Claude、Gemini 等协议特性。
- 保留原始请求体，确保出口翻译时仍有完整依据。
- 对不兼容字段明确记录为丢弃、降级、近似或报错。

一句话总结：

> 使用“源协议标注 + 原始体保留 + 轻量归一化 Envelope + 出口按目标协议裁剪”的架构，而不是两两协议互转，也不是超级中间协议。

---

## 核心原则

### 1. 不做大一统协议

不要设计一个试图完整表达所有协议能力的超级协议。各协议之间不是字段名不同，而是语义模型不同：

- OpenAI Chat Completions 使用 `messages`、`tools`、`tool_choice`、`response_format`。
- OpenAI Responses 使用 `input`、`instructions`、`previous_response_id`、`reasoning`、`text.format`。
- Claude Messages 使用顶层 `system`、`messages.content[]`、`thinking`、`tool_use`。
- Gemini 使用 `contents[]`、`parts[]`、`systemInstruction`、`generationConfig`。
- Azure OpenAI 还存在 deployment 与 model 的差异。

中间层只负责轻量归一化、能力标注、内部策略承载和损耗记录，不负责表达所有协议的完整细节。

### 2. 保留原始请求

入口适配器必须保留完整原始请求：

```ts
original: {
  headers: Record<string, string>;
  path: string;
  query: Record<string, string>;
  body: unknown;
}
```

这样出口翻译时可以根据 `source.protocol`、`target.protocol` 和 `original.body` 做更准确的定向转换。

### 3. 中间层服务项目特色能力

项目自己的特色能力不应绑死在某个外部协议结构上，而应放入 `internal` 上下文。

例如智能路由、模型别名改写、失败回退、协议补偿、审计标签、兼容模式、调试链路等，都应挂在 `internal` 下，而不是寄生在 OpenAI/Claude/Gemini 的原始 JSON 结构里。

### 4. 不兼容必须可观测

协议转换中不可避免会遇到不兼容字段。不能静默丢弃，应统一记录：

- `drop`：目标协议不支持，直接丢弃。
- `downgrade`：降级成目标协议可表达的形式。
- `approximate`：近似转换。
- `move_to_metadata`：转移到 metadata 或 internal 中。
- `error`：严格模式下直接报错。

---

## 总体架构

```mermaid
flowchart TD
  A["Client Request"] --> B["Protocol Detector"]
  B --> C["Ingress Adapter"]
  C --> D["RequestEnvelope"]
  D --> E["Auth / Access Key"]
  E --> F["Model Normalize"]
  F --> G["Routing / Pool Selection"]
  G --> H["Internal Features"]
  H --> I["Target Protocol Resolver"]
  I --> J["Egress Adapter"]
  J --> K["Provider Request"]
  K --> L["Provider Response"]
  L --> M["Response Ingress Adapter"]
  M --> N["ResponseEnvelope"]
  N --> O["Response Egress Adapter"]
  O --> P["Client Response"]
```

请求链路：

```text
客户端协议 -> 入口适配器 -> RequestEnvelope -> 路由/策略/特色能力 -> 出口适配器 -> 上游 Provider 协议
```

响应链路：

```text
上游 Provider 协议 -> 响应入口适配器 -> ResponseEnvelope -> 响应出口适配器 -> 客户端协议
```

---

## 协议分类

建议先支持以下协议：

```ts
type Protocol =
  | "openai_chat"
  | "openai_responses"
  | "azure_openai_chat"
  | "azure_openai_responses"
  | "claude_messages"
  | "gemini_generate_content"
  | "unknown";
```

协议识别可基于路由、请求体和 Header 综合判断。

| 协议 | 典型路由 | 主要特征 |
| --- | --- | --- |
| OpenAI Chat | `/v1/chat/completions` | body 有 `messages` |
| OpenAI Responses | `/v1/responses` | body 有 `input` |
| Azure Chat | `/openai/deployments/{deployment}/chat/completions` | path 有 `deployments` |
| Azure Responses | `/openai/deployments/{deployment}/responses` | path 有 `deployments` + `responses` |
| Claude Messages | `/v1/messages` | body 有 `messages`，顶层可能有 `system` |
| Gemini GenerateContent | `/v1beta/models/{model}:generateContent` | body 有 `contents` |
| Gemini Stream | `/streamGenerateContent` | streaming endpoint |

---

## RequestEnvelope

```ts
interface RequestEnvelope {
  id: string;
  source: ProtocolEndpoint;
  target?: ProtocolEndpoint;
  original: OriginalRequest;
  normalized: NormalizedRequest;
  capabilities: RequestCapabilities;
  compatibility: CompatibilityState;
  internal: InternalContext;
  timing: {
    receivedAt: number;
  };
}
```

### ProtocolEndpoint

```ts
interface ProtocolEndpoint {
  protocol: Protocol;
  vendor?: "openai" | "azure" | "anthropic" | "google" | "custom";
  apiType?: "openai" | "responses" | "claude" | "gemini" | "azure" | "custom";
  routeKind?: "chat" | "responses" | "messages" | "generateContent" | "models";
  model?: string;
  deployment?: string;
}
```

示例：

```ts
source: {
  protocol: "openai_responses",
  vendor: "openai",
  routeKind: "responses",
  model: "gpt-5.1"
}
```

```ts
target: {
  protocol: "claude_messages",
  vendor: "anthropic",
  routeKind: "messages",
  model: "claude-sonnet-4-5"
}
```

---

## NormalizedRequest

NormalizedRequest 是轻量归一化结果，只承载中间层必须理解的公共语义。

```ts
interface NormalizedRequest {
  model?: string;
  input: NormalizedInput;
  instructions?: NormalizedInstruction[];
  stream: boolean;
  tools: NormalizedTool[];
  toolChoice?: NormalizedToolChoice;
  generation: NormalizedGenerationConfig;
  responseFormat?: NormalizedResponseFormat;
  reasoning?: NormalizedReasoningConfig;
  conversation?: NormalizedConversationState;
  metadata: Record<string, unknown>;
}
```

### NormalizedInput

```ts
interface NormalizedInput {
  messages: NormalizedMessage[];
}
```

```ts
interface NormalizedMessage {
  role: "system" | "developer" | "user" | "assistant" | "tool" | "model" | "unknown";
  content: NormalizedContentPart[];
  name?: string;
  toolCallId?: string;
  sourceRole: string;
  sourceIndex: number;
  metadata?: Record<string, unknown>;
}
```

```ts
type NormalizedContentPart =
  | TextPart
  | ImagePart
  | AudioPart
  | VideoPart
  | FilePart
  | ToolCallPart
  | ToolResultPart
  | ReasoningPart
  | UnknownPart;
```

```ts
interface TextPart {
  type: "text";
  text: string;
}

interface ImagePart {
  type: "image";
  mimeType?: string;
  url?: string;
  base64?: string;
  detail?: "low" | "high" | "auto";
}

interface ToolCallPart {
  type: "tool_call";
  id: string;
  name: string;
  arguments: unknown;
}

interface ToolResultPart {
  type: "tool_result";
  toolCallId: string;
  content: NormalizedContentPart[];
  isError?: boolean;
}

interface UnknownPart {
  type: "unknown";
  sourceType: string;
  raw: unknown;
}
```

`UnknownPart` 很重要。遇到暂不支持的内容不要入口阶段直接丢弃，应保留给出口适配器判断是否可转换、降级、丢弃或报错。

---

## Instructions

不同协议的系统提示位置不同：

| 协议 | 系统提示位置 |
| --- | --- |
| OpenAI Chat | `messages[{ role: "system" }]` 或 `developer` |
| OpenAI Responses | `instructions` 或 `input` |
| Claude | 顶层 `system` |
| Gemini | `systemInstruction` |

建议抽象成：

```ts
interface NormalizedInstruction {
  role: "system" | "developer" | "policy";
  content: NormalizedContentPart[];
  source: "body" | "injected" | "internal";
  priority: number;
}
```

项目自己的系统提示注入应加入 `instructions`，不要直接改某个协议的原始 messages。出口适配器再负责落到目标协议支持的位置。

---

## GenerationConfig

```ts
interface NormalizedGenerationConfig {
  temperature?: number;
  topP?: number;
  topK?: number;
  maxOutputTokens?: number;
  stop?: string[];
  seed?: number;
  presencePenalty?: number;
  frequencyPenalty?: number;
  logprobs?: boolean;
  topLogprobs?: number;
}
```

| 字段 | OpenAI Chat | Responses | Claude | Gemini |
| --- | --- | --- | --- | --- |
| temperature | 支持 | 支持 | 支持 | 支持 |
| top_p | 支持 | 支持 | 支持 | 支持 |
| top_k | 不支持 | 不支持 | 支持 | 支持 |
| max tokens | `max_tokens` / `max_completion_tokens` | `max_output_tokens` | `max_tokens` | `maxOutputTokens` |
| stop | 支持 | 支持 | 支持 | 支持 |
| seed | 部分支持 | 部分支持 | 不支持 | 不支持 |
| logprobs | 支持程度依模型而定 | 支持程度依模型而定 | 不支持 | 不支持 |

目标协议不支持的字段应写入 `compatibility.losses`。

---

## ReasoningConfig

```ts
interface NormalizedReasoningConfig {
  enabled?: boolean;
  effort?: "minimal" | "low" | "medium" | "high";
  budgetTokens?: number;
  summary?: "auto" | "concise" | "detailed" | "none";
  raw?: unknown;
}
```

| 来源 | 目标 | 处理建议 |
| --- | --- | --- |
| Responses reasoning | OpenAI Chat | 目标模型支持才转，否则丢弃并记录 |
| Claude thinking | Responses | 可转为 `reasoning` |
| Gemini thinkingConfig | Responses | 可转为 `effort` 或 `budgetTokens` |
| OpenAI Chat 无 reasoning | Claude | 不主动注入 |

---

## Tools

```ts
interface NormalizedTool {
  type: "function" | "web_search" | "code_interpreter" | "computer_use" | "unknown";
  name: string;
  description?: string;
  inputSchema?: unknown;
  sourceType: string;
  raw?: unknown;
}
```

```ts
interface NormalizedToolChoice {
  mode: "auto" | "none" | "required" | "specific";
  name?: string;
  raw?: unknown;
}
```

| 来源 | 目标 | 策略 |
| --- | --- | --- |
| OpenAI Chat function tool | Claude | 转 Claude `tools` |
| Claude tool | OpenAI Chat | 转 `tools: [{ type: "function" }]` |
| Gemini functionDeclarations | OpenAI Chat | 转 function tool |
| Responses built-in tools | Chat | 不支持的 built-in tool 丢弃或报错 |
| Web search / file search / computer use | 任意目标 | 依能力矩阵判断 |

---

## ResponseFormat

```ts
interface NormalizedResponseFormat {
  type: "text" | "json_object" | "json_schema";
  schema?: unknown;
  strict?: boolean;
  raw?: unknown;
}
```

| 目标协议 | 映射方式 |
| --- | --- |
| OpenAI Chat | `response_format` |
| OpenAI Responses | `text.format` |
| Claude | 可转工具 schema 或提示词约束 |
| Gemini | `generationConfig.responseMimeType` + `responseSchema` |

Claude 对严格 JSON Schema 的直接支持较弱，应记录降级：

```ts
compatibility.losses.push({
  field: "responseFormat.schema",
  sourceProtocol: "openai_chat",
  targetProtocol: "claude_messages",
  reason: "target protocol does not support strict json_schema directly",
  action: "downgrade"
});
```

---

## ConversationState

```ts
interface NormalizedConversationState {
  previousResponseId?: string;
  conversationId?: string;
  store?: boolean;
  raw?: unknown;
}
```

| 来源 | 目标 | 策略 |
| --- | --- | --- |
| Responses | Responses | 保留 `previous_response_id` |
| Responses | Chat | 丢弃或由内部会话系统补齐 |
| Responses | Claude | 丢弃或转 internal metadata |
| Chat | Responses | 无 `previous_response_id`，正常为空 |

---

## CompatibilityState

```ts
interface CompatibilityState {
  mode: "strict" | "lossy" | "best_effort";
  losses: CompatibilityLoss[];
  warnings: CompatibilityWarning[];
  unsupported: UnsupportedFeature[];
}
```

```ts
interface CompatibilityLoss {
  field: string;
  sourceProtocol: Protocol;
  targetProtocol: Protocol;
  reason: string;
  action: "drop" | "downgrade" | "approximate" | "move_to_metadata";
}
```

```ts
interface UnsupportedFeature {
  feature: string;
  severity: "error" | "warning";
  message: string;
}
```

三种兼容模式：

| 模式 | 行为 |
| --- | --- |
| `strict` | 只要出现不可兼容字段就报错 |
| `lossy` | 允许丢弃或降级，但必须记录日志 |
| `best_effort` | 尽量转换，不兼容就近似或转提示 |

默认建议使用 `lossy`，调试和企业严格场景使用 `strict`。

---

## InternalContext

`InternalContext` 用来承载 API Switch 自己的中间层特色能力。

```ts
interface InternalContext {
  requestId: string;

  auth?: {
    accessKeyId?: string;
    accessKeyName?: string;
    tokenName?: string;
  };

  routing: {
    requestedModel?: string;
    normalizedModel?: string;
    selectedModel?: string;
    selectedEntryId?: string;
    selectedChannelId?: string;
    selectedApiType?: string;
    groupName?: string;
    routeReason?: string;
    failoverAttempt?: number;
  };

  policy: {
    routePolicy?: string;
    sortMode?: "custom" | "latest" | "fastest";
    compatibilityMode?: "strict" | "lossy" | "best_effort";
    allowToolDowngrade?: boolean;
    allowMultimodalDowngrade?: boolean;
  };

  features: {
    injectSystemPrompt?: boolean;
    modelAliasRewrite?: boolean;
    autoFallback?: boolean;
    translateProtocol?: boolean;
    showConversationModel?: boolean;
  };

  audit: {
    labels: string[];
    notes: string[];
  };

  debug?: {
    ingressProtocolDetectedBy?: string;
    selectedTranslator?: string;
    translationSteps?: string[];
  };
}
```

原则：

- 特色能力依赖 `internal + normalized`。
- 协议兼容依赖 `source + target + original`。
- 不可丢的信息放 `original`。
- 内部决策痕迹放 `internal`。

---

## 入口适配器

```ts
interface IngressAdapter {
  detect(req: HttpRequest): boolean;
  parse(req: HttpRequest): Promise<RequestEnvelope>;
}
```

### OpenAI Chat Ingress

| OpenAI Chat | Envelope |
| --- | --- |
| `model` | `normalized.model` |
| `messages` | `normalized.input.messages` |
| `stream` | `normalized.stream` |
| `tools` | `normalized.tools` |
| `tool_choice` | `normalized.toolChoice` |
| `response_format` | `normalized.responseFormat` |
| `temperature/top_p/stop` | `normalized.generation` |

### OpenAI Responses Ingress

| Responses | Envelope |
| --- | --- |
| `model` | `normalized.model` |
| `instructions` | `normalized.instructions` |
| `input` | `normalized.input.messages` |
| `reasoning` | `normalized.reasoning` |
| `tools` | `normalized.tools` |
| `text.format` | `normalized.responseFormat` |
| `previous_response_id` | `normalized.conversation.previousResponseId` |
| `stream` | `normalized.stream` |

注意：Responses 的 `input` 可能是 string，也可能是 array；array 内可能包含 message、tool result、多模态内容，不能简单等同于 Chat 的 `messages`。

### Claude Messages Ingress

| Claude | Envelope |
| --- | --- |
| `model` | `normalized.model` |
| `system` | `normalized.instructions` |
| `messages` | `normalized.input.messages` |
| `tools` | `normalized.tools` |
| `tool_choice` | `normalized.toolChoice` |
| `thinking` | `normalized.reasoning` |
| `max_tokens` | `normalized.generation.maxOutputTokens` |
| `temperature/top_p/top_k` | `normalized.generation` |

### Gemini Ingress

| Gemini | Envelope |
| --- | --- |
| path model | `normalized.model` |
| `contents` | `normalized.input.messages` |
| `systemInstruction` | `normalized.instructions` |
| `tools.functionDeclarations` | `normalized.tools` |
| `generationConfig` | `normalized.generation` |
| `safetySettings` | `metadata.safetySettings` |
| `cachedContent` | `metadata.cachedContent` |

---

## 出口适配器

```ts
interface EgressAdapter {
  supports(target: ProtocolEndpoint): boolean;
  buildRequest(envelope: RequestEnvelope): Promise<HttpRequest>;
}
```

出口适配器根据以下信息生成目标协议请求：

- `source.protocol`
- `target.protocol`
- `original.body`
- `normalized`
- `compatibility.mode`
- `internal.policy`

### Responses -> OpenAI Chat

可保留：

| Responses | OpenAI Chat |
| --- | --- |
| `model` | `model` |
| `input` | `messages` |
| `instructions` | system/developer message |
| function tools | `tools` |
| `tool_choice` | `tool_choice` |
| `stream` | `stream` |
| `temperature/top_p` | 对应字段 |

应丢弃或降级：

| Responses 字段 | 处理 |
| --- | --- |
| `previous_response_id` | Chat 不支持，丢弃或转 internal |
| `store` | Chat 不支持，丢弃 |
| `reasoning` | 目标 Chat 模型不支持则丢弃 |
| built-in tools | Chat 不支持的工具丢弃或报错 |
| `text.format` | 转 `response_format`，不支持则降级 |
| `include` | 多数 Chat 不支持，丢弃 |

### OpenAI Chat -> Responses

| Chat | Responses |
| --- | --- |
| `model` | `model` |
| `messages` | `input` |
| system/developer messages | `instructions` 或 `input` |
| `tools` | `tools` |
| `tool_choice` | `tool_choice` |
| `response_format` | `text.format` |
| `stream` | `stream` |

Chat 的 `messages` 应整体转换为 Responses `input` array，不应简单拼成字符串。

### Claude -> OpenAI Chat

| Claude | OpenAI Chat |
| --- | --- |
| `system` | system message |
| `messages` | `messages` |
| content text | text content |
| image content | image content |
| `tools` | function tools |
| `tool_choice` | `tool_choice` |
| `thinking` | 目标支持才转，否则丢弃 |
| `max_tokens` | `max_tokens` 或 `max_completion_tokens` |

Claude 的 `tool_use` content block 需要专门转成 OpenAI Chat 的 `tool_calls`。

### OpenAI Chat -> Claude

| OpenAI Chat | Claude |
| --- | --- |
| system message | 顶层 `system` |
| user/assistant messages | `messages` |
| tool_calls | assistant content block `tool_use` |
| tool result | user content block `tool_result` |
| tools | Claude `tools` |
| response_format | 降级为 system instruction 或 tool schema |

Claude 不接受 `system` 作为普通 message，应提升到顶层。

### Gemini -> OpenAI Chat

| Gemini | OpenAI Chat |
| --- | --- |
| `contents[].role=user` | user message |
| `contents[].role=model` | assistant message |
| `parts[].text` | text |
| `parts[].inlineData` | image/audio/file |
| `functionCall` | tool call |
| `functionResponse` | tool result |
| `systemInstruction` | system message |

### OpenAI Chat -> Gemini

| OpenAI Chat | Gemini |
| --- | --- |
| system message | `systemInstruction` |
| user message | `contents.role=user` |
| assistant message | `contents.role=model` |
| text content | `parts.text` |
| image_url/base64 | `parts.inlineData` 或 `fileData` |
| tool_calls | `functionCall` |
| tool result | `functionResponse` |
| tools | `tools.functionDeclarations` |

---

## ResponseEnvelope

```ts
interface ResponseEnvelope {
  id: string;
  source: ProtocolEndpoint;
  target: ProtocolEndpoint;
  original: {
    status: number;
    headers: Record<string, string>;
    body?: unknown;
    stream?: unknown;
  };
  normalized: NormalizedResponse;
  compatibility: CompatibilityState;
  internal: InternalResponseContext;
  timing: {
    startedAt: number;
    firstTokenAt?: number;
    completedAt?: number;
  };
}
```

```ts
interface NormalizedResponse {
  id?: string;
  model?: string;
  output: NormalizedOutputItem[];
  stopReason?: NormalizedStopReason;
  usage?: NormalizedUsage;
  stream: boolean;
  metadata: Record<string, unknown>;
}
```

```ts
type NormalizedOutputItem =
  | ResponseMessageItem
  | ResponseToolCallItem
  | ResponseReasoningItem
  | ResponseErrorItem;
```

```ts
interface ResponseMessageItem {
  type: "message";
  role: "assistant";
  content: NormalizedContentPart[];
}

interface NormalizedUsage {
  inputTokens?: number;
  outputTokens?: number;
  totalTokens?: number;
  reasoningTokens?: number;
  cachedInputTokens?: number;
  raw?: unknown;
}
```

---

## Streaming

Streaming 不建议强行统一成某个外部协议格式。内部应使用事件流：

```ts
type StreamEvent =
  | StreamStartEvent
  | StreamTextDeltaEvent
  | StreamToolCallDeltaEvent
  | StreamReasoningDeltaEvent
  | StreamUsageEvent
  | StreamEndEvent
  | StreamErrorEvent;
```

```ts
interface StreamTextDeltaEvent {
  type: "text_delta";
  index: number;
  delta: string;
}

interface StreamToolCallDeltaEvent {
  type: "tool_call_delta";
  index: number;
  id?: string;
  name?: string;
  argumentsDelta?: string;
}

interface StreamEndEvent {
  type: "end";
  stopReason?: NormalizedStopReason;
  usage?: NormalizedUsage;
}
```

出口再转成目标协议：

| 目标协议 | 输出格式 |
| --- | --- |
| OpenAI Chat | `chat.completion.chunk` |
| OpenAI Responses | `response.output_text.delta` 等事件 |
| Claude | `message_start` / `content_block_delta` |
| Gemini | JSON chunks 或 SSE 包装 |

---

## 错误统一

```ts
interface NormalizedError {
  code: string;
  message: string;
  type:
    | "invalid_request"
    | "authentication_error"
    | "rate_limit"
    | "quota_exceeded"
    | "provider_error"
    | "timeout"
    | "unsupported_feature"
    | "translation_error";
  status?: number;
  sourceProtocol?: Protocol;
  targetProtocol?: Protocol;
  provider?: string;
  raw?: unknown;
}
```

出口错误格式：

OpenAI 风格：

```json
{
  "error": {
    "message": "...",
    "type": "invalid_request_error",
    "code": "..."
  }
}
```

Claude 风格：

```json
{
  "type": "error",
  "error": {
    "type": "invalid_request_error",
    "message": "..."
  }
}
```

Gemini 风格：

```json
{
  "error": {
    "code": 400,
    "message": "...",
    "status": "INVALID_ARGUMENT"
  }
}
```

---

## 能力矩阵

目标协议能力矩阵用于判断某个字段是否可转换。

```ts
interface ProtocolCapabilities {
  protocol: Protocol;

  input: {
    text: boolean;
    image: boolean;
    audio: boolean;
    video: boolean;
    file: boolean;
  };

  tools: {
    functionCalling: boolean;
    parallelToolCalls: boolean;
    builtInTools: string[];
  };

  generation: {
    temperature: boolean;
    topP: boolean;
    topK: boolean;
    stop: boolean;
    seed: boolean;
    logprobs: boolean;
  };

  reasoning: {
    supported: boolean;
    effort: boolean;
    budgetTokens: boolean;
    visibleThinking: boolean;
  };

  responseFormat: {
    jsonObject: boolean;
    jsonSchema: boolean;
    strictSchema: boolean;
  };

  conversation: {
    previousResponseId: boolean;
    serverSideState: boolean;
  };

  streaming: {
    textDelta: boolean;
    toolDelta: boolean;
    usageInStream: boolean;
  };
}
```

示例：

```ts
const OPENAI_CHAT_CAPABILITIES: ProtocolCapabilities = {
  protocol: "openai_chat",
  input: {
    text: true,
    image: true,
    audio: false,
    video: false,
    file: false,
  },
  tools: {
    functionCalling: true,
    parallelToolCalls: true,
    builtInTools: [],
  },
  generation: {
    temperature: true,
    topP: true,
    topK: false,
    stop: true,
    seed: true,
    logprobs: true,
  },
  reasoning: {
    supported: false,
    effort: false,
    budgetTokens: false,
    visibleThinking: false,
  },
  responseFormat: {
    jsonObject: true,
    jsonSchema: true,
    strictSchema: true,
  },
  conversation: {
    previousResponseId: false,
    serverSideState: false,
  },
  streaming: {
    textDelta: true,
    toolDelta: true,
    usageInStream: true,
  },
};
```

---

## 翻译决策流程

```mermaid
flowchart TD
  A["RequestEnvelope"] --> B["Resolve Target Protocol"]
  B --> C["Load Source Capabilities"]
  C --> D["Load Target Capabilities"]
  D --> E["Compare Required Features"]
  E --> F{"Compatible?"}
  F -->|Yes| G["Build Target Request"]
  F -->|No| H{"Compatibility Mode"}
  H -->|strict| I["Return Unsupported Feature Error"]
  H -->|lossy| J["Drop / Downgrade + Record Loss"]
  H -->|best_effort| K["Approximate + Warning"]
  J --> G
  K --> G
```

---

## 示例：Responses 输入，OpenAI Chat 出口

原始请求：

```json
{
  "model": "gpt-5.1",
  "instructions": "You are a translator.",
  "input": [
    {
      "role": "user",
      "content": [
        {
          "type": "input_text",
          "text": "Translate hello"
        }
      ]
    }
  ],
  "reasoning": {
    "effort": "medium"
  },
  "previous_response_id": "resp_123",
  "stream": true
}
```

出口 OpenAI Chat：

```json
{
  "model": "gpt-4o",
  "messages": [
    {
      "role": "system",
      "content": "You are a translator."
    },
    {
      "role": "user",
      "content": "Translate hello"
    }
  ],
  "stream": true
}
```

记录损耗：

```json
[
  {
    "field": "reasoning",
    "sourceProtocol": "openai_responses",
    "targetProtocol": "openai_chat",
    "reason": "target protocol does not support reasoning config",
    "action": "drop"
  },
  {
    "field": "previous_response_id",
    "sourceProtocol": "openai_responses",
    "targetProtocol": "openai_chat",
    "reason": "target protocol does not support server-side response continuation",
    "action": "drop"
  }
]
```

---

## Rust 模块建议

建议后端按如下结构拆分：

```text
src-tauri/src/protocol/
  mod.rs
  types.rs
  detect.rs
  compatibility.rs
  capabilities.rs
  stream.rs
  error.rs

  ingress/
    mod.rs
    openai_chat.rs
    openai_responses.rs
    claude.rs
    gemini.rs
    azure.rs

  egress/
    mod.rs
    openai_chat.rs
    openai_responses.rs
    claude.rs
    gemini.rs
    azure.rs
```

核心 trait：

```rust
pub trait IngressAdapter {
    fn protocol(&self) -> Protocol;
    fn detect(&self, req: &HttpRequestParts, body: &serde_json::Value) -> bool;
    fn parse(&self, req: HttpRequestParts, body: serde_json::Value) -> Result<RequestEnvelope, ProtocolError>;
}
```

```rust
pub trait EgressAdapter {
    fn protocol(&self) -> Protocol;
    fn build(&self, envelope: &RequestEnvelope) -> Result<ProviderRequest, ProtocolError>;
}
```

---

## 落地路线

### Phase 1：协议标注 + Envelope 骨架

先实现：

- `Protocol`
- `RequestEnvelope`
- `source.protocol`
- `target.protocol`
- `original.body`
- `normalized.model`
- `normalized.stream`
- `normalized.input.messages`
- `compatibility.losses`

首批支持：

- OpenAI Chat
- OpenAI Responses

目标：先把“入口标注，出口有依据”落地。

### Phase 2：主流协议请求翻译

增加：

- Claude Messages ingress/egress
- Gemini GenerateContent ingress/egress
- Azure Chat ingress/egress

重点处理：

- messages
- system/instructions
- stream
- generation config
- model/deployment

### Phase 3：工具、多模态、结构化输出

增加：

- tools
- tool calls
- tool results
- image input
- response format
- JSON schema
- reasoning config

该阶段必须引入能力矩阵和损耗记录。

### Phase 4：Streaming 统一事件

实现：

- OpenAI Chat stream -> internal stream events
- Responses stream -> internal stream events
- Claude stream -> internal stream events
- Gemini stream -> internal stream events
- internal stream events -> 各目标协议 stream

Streaming 是复杂度最高的部分，建议最后实施。

---

## 最终建议

推荐落地架构：

```ts
interface RequestEnvelope {
  source: ProtocolEndpoint;
  target?: ProtocolEndpoint;
  original: {
    headers: Record<string, string>;
    path: string;
    query: Record<string, string>;
    body: unknown;
  };
  normalized: NormalizedRequest;
  compatibility: CompatibilityState;
  internal: InternalContext;
}
```

这套设计同时满足：

- 支持主流协议。
- 不被 OpenAI Chat 绑死。
- 不过早丢失 Responses、Claude、Gemini 的特有能力。
- 保留 API Switch 自己的中间层特色能力。
- 新增协议时避免两两互转导致组合爆炸。
- 不兼容字段可以明确记录、降级或报错。

最终原则：

> 协议转换在边界完成，平台能力留在中间；外部协议可以变化，但内部路由、策略、审计和兼容决策应稳定复用。

---

## 官方协议核验与补充缺口

本节用于对照主流官方协议要求，检查本设计是否足够包容。当前核验结果：整体 Envelope 架构可以覆盖主流协议，但需要补充若干字段族，尤其是 Responses API 的后台任务、MCP 工具、并行工具调用、截断策略，以及 Gemini 的安全配置和 tool config。

### 核验来源说明

已实际访问并核验：

- Azure OpenAI 官方 Reference 文档，覆盖 Chat Completions、Responses、Audio 等数据平面接口。
- Azure OpenAI Responses 官方 how-to 文档，覆盖 `previous_response_id`、`background`、MCP tools、`store`、streaming 等行为。

受当前网络区域限制：

- OpenAI 官方开发者文档在当前环境返回 403；已添加 `openaiDeveloperDocs` MCP，但当前会话未暴露对应工具，通常需要重启 Codex 后才能继续用官方 MCP 精确核验。
- Anthropic 官方文档在当前环境重定向到区域不可用页面。
- Google AI 官方文档当前直连超时。

因此，以下结论以已能访问的 Microsoft 官方 Azure OpenAI 文档为硬证据，并结合各厂商公开协议结构给出设计缺口。后续应在可访问官方文档的环境中复核 OpenAI、Anthropic、Google 的最新字段。

---

### 已覆盖良好的部分

| 能力 | 当前设计状态 | 结论 |
| --- | --- | --- |
| 协议来源标注 | `source.protocol` / `target.protocol` | 已覆盖 |
| 原始请求保留 | `original.body` / `original.headers` / `original.path` | 已覆盖 |
| Chat messages | `normalized.input.messages` | 已覆盖 |
| Responses input | `normalized.input.messages` + `original.body` | 基本覆盖 |
| system / instructions | `normalized.instructions` | 已覆盖 |
| stream | `normalized.stream` + `StreamEvent` | 已覆盖 |
| tools / tool_choice | `normalized.tools` / `normalized.toolChoice` | 基本覆盖 |
| response_format / text.format | `normalized.responseFormat` | 已覆盖 |
| reasoning / thinking | `normalized.reasoning` | 基本覆盖 |
| previous_response_id / store | `normalized.conversation` | 基本覆盖 |
| Azure deployment | `ProtocolEndpoint.deployment` | 已覆盖 |
| usage | `NormalizedUsage` | 基本覆盖 |
| 不兼容记录 | `CompatibilityState.losses` | 已覆盖 |

---

### 必须补充的字段族

#### 1. Responses 后台任务 background

Azure OpenAI Responses 官方文档明确支持 `background: true`，用于长时间运行任务，并通过 GET 轮询状态。

当前设计没有显式字段承载该能力。建议补充：

```ts
interface NormalizedExecutionOptions {
  background?: boolean;
  pollable?: boolean;
  timeoutMs?: number;
  serviceTier?: string;
  raw?: unknown;
}
```

并加入：

```ts
interface NormalizedRequest {
  execution?: NormalizedExecutionOptions;
}
```

兼容策略：

| 来源 | 目标 | 策略 |
| --- | --- | --- |
| Responses `background=true` | Responses | 保留 |
| Responses `background=true` | Chat / Claude / Gemini | `strict` 下报错，`lossy` 下丢弃并记录，`best_effort` 下同步执行并警告 |

---

#### 2. Responses truncation 截断策略

Azure Responses 返回和请求结构中出现 `truncation` 字段，用于控制上下文截断策略。

当前设计未显式表达。建议补充到 conversation 或 generation 中，更推荐独立放到 conversation：

```ts
interface NormalizedConversationState {
  previousResponseId?: string;
  conversationId?: string;
  store?: boolean;
  truncation?: "auto" | "disabled" | string;
  raw?: unknown;
}
```

兼容策略：

| 来源 | 目标 | 策略 |
| --- | --- | --- |
| Responses truncation | Responses | 保留 |
| Responses truncation | Chat / Claude / Gemini | 多数情况下不可直接表达，转 `internal.policy` 或记录损耗 |

---

#### 3. Responses MCP 工具

Azure Responses 官方文档明确支持远程 MCP servers，工具项 `type: "mcp"`。当前 `NormalizedTool.type` 没有 `mcp`。

建议修改：

```ts
interface NormalizedTool {
  type:
    | "function"
    | "web_search"
    | "file_search"
    | "code_interpreter"
    | "computer_use"
    | "mcp"
    | "unknown";
  name: string;
  description?: string;
  inputSchema?: unknown;
  sourceType: string;
  serverLabel?: string;
  serverUrl?: string;
  allowedTools?: string[];
  raw?: unknown;
}
```

兼容策略：

| 来源 | 目标 | 策略 |
| --- | --- | --- |
| Responses MCP | Responses | 保留 |
| Responses MCP | Chat / Claude / Gemini | 一般不可直接转，除非平台自己代理 MCP 并展开成 function tools |

如果 API Switch 后续支持 MCP 代理，可将 MCP server 暴露的 tool definitions 转换成普通 function tools，但这属于平台能力，不是单纯协议字段映射。

---

#### 4. parallel_tool_calls

Azure Responses 响应结构中包含 `parallel_tool_calls`，OpenAI/Azure Chat 也有并行工具调用语义。

当前设计只有能力矩阵里的 `parallelToolCalls`，但请求归一化没有承载该开关。建议补充：

```ts
interface NormalizedToolOptions {
  parallelToolCalls?: boolean;
  raw?: unknown;
}
```

并加入：

```ts
interface NormalizedRequest {
  toolOptions?: NormalizedToolOptions;
}
```

兼容策略：

| 来源 | 目标 | 策略 |
| --- | --- | --- |
| 支持 parallel tool calls | 支持 | 保留 |
| 支持 parallel tool calls | 不支持 | 记录降级，必要时串行化工具调用 |

---

#### 5. Responses include 字段

Responses API 存在 `include` 类字段，用于请求额外输出内容或中间信息。当前设计只在示例中说“多数 Chat 不支持，丢弃”，但没有归一化字段承载。

建议补充：

```ts
interface NormalizedOutputOptions {
  include?: string[];
  modalities?: string[];
  raw?: unknown;
}
```

并加入：

```ts
interface NormalizedRequest {
  output?: NormalizedOutputOptions;
}
```

兼容策略：

| 来源 | 目标 | 策略 |
| --- | --- | --- |
| Responses include | Responses | 保留 |
| Responses include | Chat / Claude / Gemini | 不能表达则丢弃或转 metadata，并记录损耗 |

---

#### 6. service_tier / latency tier

OpenAI/Azure 系协议中可能存在服务层级或延迟层级参数。当前设计没有明确字段。

建议放入 execution：

```ts
interface NormalizedExecutionOptions {
  background?: boolean;
  pollable?: boolean;
  timeoutMs?: number;
  serviceTier?: string;
  raw?: unknown;
}
```

---

#### 7. metadata 的双重含义

当前设计中 `metadata: Record<string, unknown>` 已存在，但需要明确区分：

- 用户请求中的 provider metadata。
- API Switch 内部 metadata。
- 协议转换产生的 compatibility metadata。

建议：

```ts
interface NormalizedRequest {
  metadata: {
    provider?: Record<string, unknown>;
    client?: Record<string, unknown>;
    translation?: Record<string, unknown>;
  };
}
```

避免把用户 metadata 和内部审计信息混在一起。

---

### Gemini 需要补强的部分

当前设计已覆盖 `contents`、`parts`、`systemInstruction`、`generationConfig`、`safetySettings`、`cachedContent`、`functionDeclarations`，但仍建议补充：

#### 1. toolConfig

Gemini 工具调用通常除 `tools` 外还有 `toolConfig`，用于控制 function calling mode、allowed function names 等。

建议让 `NormalizedToolChoice.raw` 保留完整配置，并增加：

```ts
interface NormalizedToolChoice {
  mode: "auto" | "none" | "required" | "specific";
  name?: string;
  allowedNames?: string[];
  raw?: unknown;
}
```

#### 2. safetySettings 不能只放 metadata

Gemini 的 `safetySettings` 会影响模型输出，不只是普通 metadata。建议独立为 safety：

```ts
interface NormalizedSafetySettings {
  settings: unknown[];
  raw?: unknown;
}
```

并加入：

```ts
interface NormalizedRequest {
  safety?: NormalizedSafetySettings;
}
```

兼容策略：

| 来源 | 目标 | 策略 |
| --- | --- | --- |
| Gemini safetySettings | Gemini | 保留 |
| Gemini safetySettings | OpenAI / Claude | 无等价能力，转 internal policy 或记录损耗 |

#### 3. cachedContent

`cachedContent` 是 Gemini 特有的上下文缓存引用。当前设计放 metadata 可以接受，但更建议放 conversation：

```ts
interface NormalizedConversationState {
  cachedContent?: string;
}
```

---

### Claude 需要补强的部分

当前设计覆盖 `system`、`messages`、`tools`、`tool_choice`、`thinking`、`max_tokens`。建议补充：

#### 1. stop_sequences

Claude 使用 `stop_sequences`，OpenAI/Azure Chat 使用 `stop`。当前 `NormalizedGenerationConfig.stop?: string[]` 可以覆盖，但入口适配器需要明确映射。

#### 2. metadata.user_id

Claude 常见请求字段包含 metadata，例如 `user_id`。当前 metadata 可以覆盖，但建议放入 `metadata.provider`。

#### 3. container / context management 类字段

Claude 新版协议可能存在 container、context management、memory/tool 相关字段。建议所有未识别 Claude 顶层字段先进入：

```ts
normalized.metadata.provider
```

并在 `UnknownPart` / `raw` 中保留，避免入口阶段丢失。

#### 4. thinking 输出与加密/签名块

Claude thinking/extended thinking 可能包含非普通文本块。当前 `ReasoningPart` 未展开定义，建议定义：

```ts
interface ReasoningPart {
  type: "reasoning";
  text?: string;
  signature?: string;
  encrypted?: string;
  raw?: unknown;
}
```

如果目标协议不支持 visible thinking，应按兼容模式处理。

---

### OpenAI / Azure Chat 需要补强的部分

基于 Azure OpenAI 官方 Reference，Chat Completions 还需覆盖：

#### 1. n / choices 数量

当前设计没有 `n`。建议加入 generation：

```ts
interface NormalizedGenerationConfig {
  n?: number;
}
```

兼容策略：目标协议不支持多候选时，`strict` 报错，`lossy` 降级为 1。

#### 2. user 字段

OpenAI/Azure Chat 有 `user` 字段用于最终用户标识。建议放 metadata：

```ts
metadata.client.user?: string;
```

#### 3. logit_bias

当前设计没有 `logit_bias`。建议补充：

```ts
interface NormalizedGenerationConfig {
  logitBias?: Record<string, number>;
}
```

#### 4. function_call / functions 旧字段

Azure Reference 仍列出 deprecated `functions` / `function_call`。当前设计只覆盖新 `tools` / `tool_choice`。入口适配器需要支持旧字段并映射到 normalized tools：

| Deprecated | Normalized |
| --- | --- |
| `functions` | `tools` with `type: "function"` |
| `function_call` | `toolChoice` |

#### 5. max_tokens 与 max_completion_tokens

当前设计用 `maxOutputTokens`，但要记录来源字段：

```ts
interface NormalizedGenerationConfig {
  maxOutputTokens?: number;
  maxOutputTokensSource?: "max_tokens" | "max_completion_tokens" | "max_output_tokens";
}
```

因为 reasoning 模型下 `max_completion_tokens` 包括 reasoning tokens，语义不同于老 `max_tokens`。

---

### Responses 需要补强的输出状态

Responses API 的响应不仅是一次性 message，还可能有状态生命周期：queued、in_progress、completed、failed、cancelled 等。当前 `NormalizedResponse` 没有状态字段。

建议：

```ts
interface NormalizedResponse {
  id?: string;
  model?: string;
  status?: "queued" | "in_progress" | "completed" | "failed" | "cancelled" | "incomplete" | string;
  output: NormalizedOutputItem[];
  stopReason?: NormalizedStopReason;
  usage?: NormalizedUsage;
  stream: boolean;
  metadata: Record<string, unknown>;
}
```

并为后台任务增加轮询上下文：

```ts
interface InternalResponseContext {
  poll?: {
    responseId?: string;
    nextPollAfterMs?: number;
    terminal: boolean;
  };
}
```

---

### Streaming 事件需要补充

当前 StreamEvent 已覆盖文本、工具、reasoning、usage、end、error，但建议显式补充生命周期事件：

```ts
type StreamEvent =
  | StreamStartEvent
  | StreamStatusEvent
  | StreamTextDeltaEvent
  | StreamToolCallDeltaEvent
  | StreamReasoningDeltaEvent
  | StreamUsageEvent
  | StreamEndEvent
  | StreamErrorEvent;
```

```ts
interface StreamStatusEvent {
  type: "status";
  status: "created" | "queued" | "in_progress" | "completed" | "failed" | "cancelled" | string;
  raw?: unknown;
}
```

这样可以兼容 Responses 流式事件和后台任务状态。

---

### 建议更新后的关键结构

```ts
interface NormalizedRequest {
  model?: string;
  input: NormalizedInput;
  instructions?: NormalizedInstruction[];
  stream: boolean;
  tools: NormalizedTool[];
  toolChoice?: NormalizedToolChoice;
  toolOptions?: NormalizedToolOptions;
  generation: NormalizedGenerationConfig;
  responseFormat?: NormalizedResponseFormat;
  reasoning?: NormalizedReasoningConfig;
  conversation?: NormalizedConversationState;
  execution?: NormalizedExecutionOptions;
  output?: NormalizedOutputOptions;
  safety?: NormalizedSafetySettings;
  metadata: {
    provider?: Record<string, unknown>;
    client?: Record<string, unknown>;
    translation?: Record<string, unknown>;
  };
}
```

```ts
interface NormalizedGenerationConfig {
  temperature?: number;
  topP?: number;
  topK?: number;
  maxOutputTokens?: number;
  maxOutputTokensSource?: "max_tokens" | "max_completion_tokens" | "max_output_tokens";
  stop?: string[];
  seed?: number;
  n?: number;
  presencePenalty?: number;
  frequencyPenalty?: number;
  logprobs?: boolean;
  topLogprobs?: number;
  logitBias?: Record<string, number>;
}
```

```ts
interface NormalizedTool {
  type:
    | "function"
    | "web_search"
    | "file_search"
    | "code_interpreter"
    | "computer_use"
    | "mcp"
    | "unknown";
  name: string;
  description?: string;
  inputSchema?: unknown;
  sourceType: string;
  serverLabel?: string;
  serverUrl?: string;
  allowedTools?: string[];
  raw?: unknown;
}
```

---

## 核验结论

当前设计的总体架构是正确的，能够包容主流协议的核心请求/响应模型，尤其适合 API Switch 这种中间代理产品。

但如果目标是“包容现在几个主流协议”，需要把以下能力补进正式设计：

1. Responses `background` 和响应状态生命周期。
2. Responses `truncation`。
3. Responses MCP tools。
4. `parallel_tool_calls` 请求开关。
5. Responses `include` / 输出选项。
6. Gemini `toolConfig`。
7. Gemini `safetySettings` 独立为 safety，不只当 metadata。
8. Gemini `cachedContent` 放入 conversation。
9. OpenAI/Azure Chat `n`、`user`、`logit_bias`。
10. Deprecated `functions` / `function_call` 的兼容入口。
11. `max_tokens` / `max_completion_tokens` / `max_output_tokens` 的语义来源标记。
12. ResponseEnvelope 增加 `status` 和后台任务 polling 上下文。
13. Streaming 增加状态事件。

补完这些后，设计可以比较稳地覆盖 OpenAI Chat、OpenAI Responses、Azure OpenAI、Claude Messages、Gemini GenerateContent 的主流协议要求，同时不会把平台自己的特色能力绑定到某个外部协议上。

---

## 封版期稳定计划

### 背景

当前程序已经封版，短期内不进入开发实现阶段。本计划的目标不是推动立即编码，而是把多协议 Envelope 方案稳定下来，作为后续版本开发、重构、评审和测试的架构基线。

封版期只做以下事情：

- 固化协议翻译的设计边界。
- 固化术语、数据结构、兼容策略和落地顺序。
- 明确哪些能力属于稳定核心，哪些属于后续扩展。
- 明确未来开发时不得破坏的兼容原则。
- 为下一版本开发准备验收清单，而不是修改当前封版程序。

封版期不做以下事情：

- 不改现有 Rust / TypeScript 业务代码。
- 不改现有路由逻辑。
- 不替换现有协议转换实现。
- 不调整当前 UI。
- 不改变现有用户行为。
- 不引入新的数据库字段或配置项。

---

### 稳定版设计定位

本设计文档在封版期应被视为“下一代协议翻译层设计蓝图”，而不是当前版本必须实现的功能清单。

稳定版设计的定位如下：

```text
当前封版版本：维持现状，保证稳定。
设计稳定期：冻结架构方向，补齐协议分析。
下一开发版本：按 Phase 分阶段落地。
```

也就是说，当前版本继续使用现有协议转发和转换逻辑；Envelope 方案只作为后续开发时的目标架构。

---

### 设计稳定目标

封版期要稳定以下核心结论。

#### 1. 架构方向稳定

最终方向固定为：

```text
入口协议识别 -> RequestEnvelope -> 平台内部能力 -> 目标协议出口翻译
```

不要回退到以下两种方案：

- 不要做所有协议两两互转。
- 不要把 OpenAI Chat 当成唯一内部协议。
- 不要做试图完整表达所有协议能力的超级中间协议。

稳定架构是：

```text
源协议标注 + 原始体保留 + 轻量归一化 + 兼容损耗记录 + 出口定向裁剪
```

#### 2. 数据保真原则稳定

入口阶段不得过早丢弃原始协议字段。即使某字段暂时不支持，也应放在以下位置之一：

- `original.body`
- `normalized.metadata.provider`
- `UnknownPart.raw`
- `NormalizedTool.raw`
- `NormalizedReasoningConfig.raw`
- `CompatibilityState.unsupported`

设计原则：

> 入口尽量保真，出口负责裁剪。

#### 3. 平台特色能力稳定

API Switch 自己的特色能力必须属于平台内部能力，而不是某个外部协议的副作用。

稳定承载位置：

```ts
internal: InternalContext
```

包括但不限于：

- access key 认证信息。
- 模型别名和模型重写。
- 路由策略。
- group / pool / channel 选择结果。
- sort mode。
- 失败回退。
- 兼容模式。
- 审计标签。
- 协议翻译调试链路。

未来开发时，不应把这些能力散落进 OpenAI、Responses、Claude、Gemini 的私有字段里。

#### 4. 兼容策略稳定

所有协议不兼容必须经过统一策略处理。

稳定模式：

```ts
type CompatibilityMode = "strict" | "lossy" | "best_effort";
```

三种模式语义固定：

| 模式 | 行为 |
| --- | --- |
| `strict` | 不兼容即报错，不做隐式丢弃 |
| `lossy` | 允许丢弃或降级，但必须记录损耗 |
| `best_effort` | 尽量近似转换，必要时转提示、metadata 或 internal |

默认建议：

```text
生产默认：lossy
调试/测试：strict
兼容优先：best_effort
```

#### 5. 分阶段落地稳定

封版后下一开发版本必须按阶段推进，不要一次性重写全部协议层。

稳定落地顺序：

1. Phase 0：文档稳定与协议样本收集。
2. Phase 1：Envelope 骨架与协议标注。
3. Phase 2：OpenAI Chat 与 Responses 双向翻译。
4. Phase 3：Claude / Gemini / Azure 扩展。
5. Phase 4：工具、多模态、结构化输出。
6. Phase 5：Streaming 统一事件。
7. Phase 6：兼容性测试矩阵与回归固化。

---

## 封版期详细设计任务

封版期的任务不是开发，而是把设计沉淀到足够可执行。建议按以下任务推进。

### Task 1：术语冻结

需要固定以下术语，后续代码、文档、测试都使用同一套名称。

| 术语 | 含义 |
| --- | --- |
| `source protocol` | 客户端进入 API Switch 时使用的协议 |
| `target protocol` | API Switch 转发给上游 Provider 时使用的协议 |
| `Ingress Adapter` | 入口协议解析器 |
| `Egress Adapter` | 出口协议构造器 |
| `RequestEnvelope` | 请求中间上下文包 |
| `ResponseEnvelope` | 响应中间上下文包 |
| `NormalizedRequest` | 轻量归一化请求 |
| `InternalContext` | API Switch 平台内部上下文 |
| `CompatibilityState` | 协议兼容状态与损耗记录 |
| `Capability Matrix` | 协议能力矩阵 |
| `Loss` | 转换过程中发生的信息损耗 |

禁止混用以下概念：

- `source protocol` 不等于 `provider api_type`。
- `target protocol` 不等于 `model name`。
- `normalized` 不等于 `OpenAI Chat messages`。
- `metadata` 不等于 `internal`。
- `lossy` 不等于“随便丢字段”。

---

### Task 2：协议样本冻结

封版期应收集但不实现一组协议样本，作为未来开发测试基线。

建议创建未来目录：

```text
docs/protocol-samples/
  openai-chat/
  openai-responses/
  azure-openai/
  claude-messages/
  gemini-generate-content/
```

当前封版期可以只在文档中定义样本分类，不一定创建文件。

每个协议至少准备以下样本：

| 样本类型 | 目的 |
| --- | --- |
| simple text | 验证最小文本请求 |
| system instruction | 验证系统提示位置转换 |
| stream | 验证流式字段和响应事件 |
| function tool | 验证工具定义转换 |
| tool call result | 验证工具调用结果转换 |
| image input | 验证多模态输入 |
| json schema output | 验证结构化输出 |
| reasoning / thinking | 验证推理参数与输出 |
| unsupported feature | 验证 strict / lossy / best_effort |

每个样本应包含：

```text
source_protocol
request_path
request_headers
request_body
expected_normalized_summary
expected_target_body
expected_losses
```

---

### Task 3：能力矩阵冻结

封版期应把能力矩阵作为设计基线，未来开发时按矩阵判断是否可转换。

最低需要维护以下维度：

```ts
interface ProtocolCapabilities {
  protocol: Protocol;
  input: InputCapabilities;
  tools: ToolCapabilities;
  generation: GenerationCapabilities;
  reasoning: ReasoningCapabilities;
  responseFormat: ResponseFormatCapabilities;
  conversation: ConversationCapabilities;
  execution: ExecutionCapabilities;
  safety: SafetyCapabilities;
  streaming: StreamingCapabilities;
}
```

新增能力维度：

```ts
interface ExecutionCapabilities {
  background: boolean;
  polling: boolean;
  serviceTier: boolean;
}
```

```ts
interface SafetyCapabilities {
  safetySettings: boolean;
  moderationHints: boolean;
}
```

```ts
interface ConversationCapabilities {
  previousResponseId: boolean;
  serverSideState: boolean;
  cachedContent: boolean;
  truncation: boolean;
}
```

封版期先稳定字段，不要求填完所有协议的精确值。后续开发前再依据官方文档逐项核实。

---

### Task 4：损耗记录规范冻结

所有损耗记录必须可用于日志、调试、测试断言。

稳定结构：

```ts
interface CompatibilityLoss {
  field: string;
  sourceProtocol: Protocol;
  targetProtocol: Protocol;
  reason: string;
  action: "drop" | "downgrade" | "approximate" | "move_to_metadata";
  severity?: "info" | "warning" | "error";
  sourceValuePreview?: string;
}
```

损耗记录要求：

- `field` 必须是来源协议字段路径，例如 `reasoning.effort`、`previous_response_id`。
- `reason` 必须说明目标协议为什么无法表达。
- `action` 必须说明实际处理方式。
- `strict` 模式下严重损耗应转为错误。
- `lossy` 模式下至少记录 warning。
- `best_effort` 模式下应记录 approximate 或 downgrade。

示例：

```json
{
  "field": "previous_response_id",
  "sourceProtocol": "openai_responses",
  "targetProtocol": "openai_chat",
  "reason": "target protocol does not support server-side response continuation",
  "action": "drop",
  "severity": "warning"
}
```

---

### Task 5：稳定版兼容边界

封版期必须明确哪些能力未来第一版要支持，哪些先只保留原始数据。

#### 第一开发版必须支持

- OpenAI Chat -> OpenAI Chat 直通。
- OpenAI Responses -> OpenAI Responses 直通。
- OpenAI Chat -> OpenAI Responses 基础转换。
- OpenAI Responses -> OpenAI Chat 基础转换。
- `messages` / `input` 文本转换。
- `system` / `instructions` 转换。
- `stream` 标记传递。
- `temperature` / `top_p` / `max tokens` 基础参数。
- 基础 function tools。
- `CompatibilityState.losses`。

#### 第一开发版应保留但可不转换

- `background`。
- MCP tools。
- `include`。
- `truncation`。
- Gemini `safetySettings`。
- Gemini `cachedContent`。
- Claude extended thinking 特殊块。
- audio / video / file 输入。
- strict JSON schema 的跨协议强保证。

#### 第一开发版不建议处理

- 完整多协议 streaming 互转。
- 后台任务轮询代理。
- 内置 web_search / file_search / computer_use 的跨厂商等价转换。
- 所有协议的所有多模态细节。
- 多候选 `n > 1` 的跨协议保持。

原则：

> 第一开发版只处理确定、安全、可测试的转换；复杂能力先保留原始数据并记录 unsupported。

---

### Task 6：未来代码边界冻结

后续开发时建议模块边界固定如下：

```text
src-tauri/src/protocol/
  types.rs              # Envelope 和核心类型
  detect.rs             # 协议检测
  capabilities.rs       # 能力矩阵
  compatibility.rs      # 损耗与兼容策略
  error.rs              # 协议错误
  stream.rs             # 内部流事件

  ingress/
    openai_chat.rs
    openai_responses.rs
    claude.rs
    gemini.rs
    azure.rs

  egress/
    openai_chat.rs
    openai_responses.rs
    claude.rs
    gemini.rs
    azure.rs
```

现有代理转发模块未来只应调用协议层，不应继续内联大量协议转换逻辑。

目标依赖方向：

```text
proxy handlers -> protocol facade -> ingress/egress adapters
```

禁止依赖方向：

```text
ingress adapter -> proxy handlers
egress adapter -> database
protocol types -> concrete provider HTTP client
```

协议层应该保持纯转换逻辑，路由和数据库选择留在 proxy/router/service 层。

---

### Task 7：封版期评审清单

在进入下一开发版本前，应完成以下设计评审。

| 检查项 | 状态要求 |
| --- | --- |
| 是否仍保留原始请求体 | 必须是 |
| 是否避免 OpenAI Chat 内部协议化 | 必须是 |
| 是否有 source / target 标注 | 必须是 |
| 是否有损耗记录 | 必须是 |
| 是否有 strict / lossy / best_effort | 必须是 |
| 是否有能力矩阵 | 必须是 |
| 是否定义 streaming 内部事件 | 必须是 |
| 是否区分 provider metadata 和 internal context | 必须是 |
| 是否为复杂能力保留 raw | 必须是 |
| 是否定义分阶段落地 | 必须是 |

如果任一项不满足，不应开始大规模编码。

---

## 稳定版实施路线

### Phase 0：封版设计稳定

当前阶段，只做文档稳定。

交付物：

- `PROTOCOL_ENVELOPE_DESIGN.md`。
- 协议缺口清单。
- 分阶段落地计划。
- 后续官方文档复核清单。

验收标准：

- 架构方向明确。
- 核心字段稳定。
- 不兼容策略明确。
- 暂不开发边界明确。

### Phase 1：开发前准备

等程序进入下一开发周期后再启动。

交付物：

- 协议样本文件。
- golden test 预期结果。
- 能力矩阵初始实现。
- Envelope Rust 类型草案。

验收标准：

- 不接入真实转发链路。
- 只做纯转换单元测试。
- 不影响现有用户行为。

### Phase 2：灰度接入 OpenAI Chat / Responses

交付物：

- OpenAI Chat ingress / egress。
- OpenAI Responses ingress / egress。
- Chat <-> Responses 基础转换。
- compatibility losses 日志。

验收标准：

- 默认仍可走旧路径。
- 新路径可通过配置或编译开关启用。
- 出问题可快速回退。

### Phase 3：扩展 Claude / Gemini / Azure

交付物：

- Claude Messages adapter。
- Gemini GenerateContent adapter。
- Azure deployment-aware adapter。
- 协议能力矩阵完善。

验收标准：

- 每个协议至少覆盖 simple text、system instruction、stream flag、basic tools。
- 复杂能力先 unsupported，不强行转换。

### Phase 4：高级能力

交付物：

- Tool call / tool result 完整转换。
- JSON schema / response format 转换。
- 多模态 image input 转换。
- reasoning / thinking 映射。

验收标准：

- 每类高级能力都有 strict / lossy / best_effort 测试。
- 所有损耗可在日志和调试输出中观察。

### Phase 5：Streaming 统一事件

交付物：

- 内部 StreamEvent。
- 各协议 stream ingress。
- 各协议 stream egress。
- usage / end / error 状态统一。

验收标准：

- 不破坏现有 SSE 行为。
- 支持中途错误映射。
- 支持工具调用 delta。
- 支持 Responses 状态事件。

---

## 风险与控制

### 风险 1：设计过大，落地困难

控制方式：

- 第一开发版只做 Chat / Responses。
- Claude / Gemini / Azure 放后续阶段。
- 多模态、MCP、background 只保留 raw，不急于转换。

### 风险 2：中间层变成超级协议

控制方式：

- Normalized 只放平台必须理解的公共字段。
- 协议私有字段放 raw / metadata.provider。
- 出口适配器负责协议特性。

### 风险 3：损耗不可见

控制方式：

- 所有 drop / downgrade 都必须写入 `CompatibilityState.losses`。
- 日志中输出 request id 和 loss summary。
- 测试断言 expected losses。

### 风险 4：影响现有稳定版本

控制方式：

- 封版期不改代码。
- 下一开发版先纯转换测试。
- 再通过开关灰度接入。
- 保留旧路径回退。

### 风险 5：官方协议持续变化

控制方式：

- 能力矩阵按版本维护。
- 协议字段保留 raw。
- 官方文档复核作为开发前置步骤。
- 对未知字段默认保留，不默认丢弃。

---

## 官方文档复核清单

进入下一开发阶段前，需要重新核验以下官方文档：

| 厂商 | 需要核验 |
| --- | --- |
| OpenAI | Chat Completions request/response、Responses request/response、Responses streaming、tools、reasoning、structured outputs |
| Azure OpenAI | Chat Completions、Responses、API version、deployment path、Responses background、MCP tools |
| Anthropic Claude | Messages、streaming、tools、tool_use/tool_result、thinking、metadata、stop_sequences |
| Google Gemini | GenerateContent、streamGenerateContent、contents/parts、systemInstruction、tools/functionDeclarations、toolConfig、safetySettings、cachedContent |

复核后需要更新：

- `ProtocolCapabilities`。
- ingress 映射表。
- egress 映射表。
- unsupported feature 表。
- golden samples。

---

## 封版期最终结论

当前封版阶段应稳定设计，而不是立即实现。

稳定结论：

1. API Switch 下一代协议层采用 Envelope 架构。
2. Envelope 是轻量上下文包，不是超级中间协议。
3. 原始协议体必须保留。
4. 平台特色能力统一放在 `InternalContext`。
5. 协议不兼容必须记录到 `CompatibilityState`。
6. 默认生产模式建议为 `lossy`，但损耗必须可观测。
7. 后续开发必须分阶段灰度接入，不得一次性替换现有转发链路。
8. 在程序封版期间，不修改现有运行逻辑。

一句话总结：

> 现在把方案写稳、边界定稳、风险控稳；等下一开发周期，再从 OpenAI Chat / Responses 的最小闭环开始落地。

---

## 开发前验证风险点测试

本章节定义“进入开发前必须完成的验证测试设计”。由于当前程序处于封版状态，本阶段不执行代码改造，但必须先把测试目标、测试边界、样本要求、判定标准写清楚，避免后续开发时出现“设计上看起来能包容，实际落地却在数据流某一环断掉”的问题。

本章节遵循本项目的联动一致性检查规则：

1. 不能只做静态字段对照。
2. 必须覆盖协议入口、Envelope 中间态、出口请求、响应回译、流式事件、损耗记录。
3. 只要任一环可能失败，就不能宣称“已兼容”。

---

### 一、验证目标

开发前验证测试的目标不是验证业务功能，而是验证这套设计是否具备“可安全落地”的前提条件。

必须回答以下问题：

1. 是否真的覆盖了主流协议的核心请求/响应形态？
2. 是否存在设计上遗漏的顶层字段、内容块类型或生命周期状态？
3. 是否能在不丢失关键语义的前提下完成 ingress -> envelope -> egress？
4. 当目标协议不支持某特性时，是否能稳定进入 `strict / lossy / best_effort` 之一？
5. 是否能明确知道哪些能力第一版支持、哪些只能保留 raw、哪些必须报错？

---

### 二、测试分层

开发前验证测试分为六层。

| 层级 | 名称 | 目标 |
| --- | --- | --- |
| L1 | 官方协议核验 | 确认设计字段与官方协议一致 |
| L2 | 协议样本核验 | 确认样本覆盖了主流请求形态 |
| L3 | Ingress 归一化核验 | 确认入口协议能正确归一化到 Envelope |
| L4 | Egress 出口裁剪核验 | 确认目标协议请求生成规则明确 |
| L5 | Compatibility 风险核验 | 确认所有不兼容特性都有稳定处理策略 |
| L6 | Streaming / 生命周期核验 | 确认流式事件和后台任务状态可表达 |

只有六层都通过，才可以进入开发阶段。

---

### 三、L1 官方协议核验

#### 目标

确保设计文档中列出的字段、结构和行为模型与官方协议要求一致。

#### 核验对象

| 厂商 | 需核验范围 |
| --- | --- |
| OpenAI | Chat Completions、Responses、Structured Outputs、Tools、Reasoning、Streaming |
| Azure OpenAI | Chat Completions、Responses、Background tasks、MCP tools、API version、deployment path |
| Anthropic Claude | Messages、Streaming、Tools、Tool use/result、Thinking、Metadata、Stop sequences |
| Google Gemini | GenerateContent、streamGenerateContent、contents/parts、systemInstruction、functionDeclarations、toolConfig、safetySettings、cachedContent |

#### 必测项

每个协议至少核验以下内容：

- 顶层请求字段。
- 顶层响应字段。
- 输入内容块类型。
- 工具定义格式。
- 工具调用结果格式。
- 流式事件类型。
- 状态生命周期字段。
- 特有能力字段。
- Deprecated 兼容字段。

#### 风险判定

| 风险级别 | 判定标准 |
| --- | --- |
| P0 | 官方存在的核心字段在设计中完全缺失 |
| P1 | 设计中有字段承载，但语义位置不对 |
| P2 | 设计能保留 raw，但无明确转换策略 |
| P3 | 设计已覆盖，仅缺细节说明 |

#### 通过标准

- 不允许存在 P0 未关闭项。
- P1 必须在文档中明确调整结构。
- P2 必须在文档中标明“第一版仅保留 raw / unsupported”。
- P3 可以进入开发期边做边细化。

---

### 四、L2 协议样本核验

#### 目标

在开发前定义完整样本矩阵，避免后续只拿最简单文本请求做测试，导致实现看似成功但复杂协议一上来就断裂。

#### 每协议最低样本集

每个协议都至少需要以下样本：

| 编号 | 样本类型 | 目的 |
| --- | --- | --- |
| S1 | simple_text | 验证最小文本请求 |
| S2 | system_instruction | 验证系统提示位置 |
| S3 | stream_basic | 验证流式标记与基础流事件 |
| S4 | tool_function | 验证 function tool 定义 |
| S5 | tool_result | 验证 tool result 回传 |
| S6 | image_input | 验证图像输入内容块 |
| S7 | json_schema_output | 验证结构化输出 |
| S8 | reasoning_or_thinking | 验证推理参数与推理输出 |
| S9 | unsupported_feature | 验证不兼容能力处理 |
| S10 | metadata_and_user | 验证 provider/client metadata |

#### 各协议额外样本

##### OpenAI Responses 额外样本

- `background_task`
- `previous_response_id_chain`
- `mcp_tool`
- `parallel_tool_calls`
- `include_and_truncation`

##### OpenAI / Azure Chat 额外样本

- `deprecated_functions`
- `n_choices`
- `logit_bias`
- `response_format_json_object`
- `response_format_json_schema`

##### Claude 额外样本

- `system_top_level`
- `tool_use_block`
- `tool_result_block`
- `thinking_block`
- `stop_sequences`

##### Gemini 额外样本

- `system_instruction_parts`
- `function_declarations`
- `tool_config`
- `safety_settings`
- `cached_content`

#### 样本模板

每个样本应定义：

```yaml
id: openai_responses.background_task
source_protocol: openai_responses
description: Responses background task request
request_path: /v1/responses
request_headers: {}
request_body: {}
expected_normalized:
  model: gpt-4.1
  stream: false
  execution.background: true
expected_target:
  protocol: openai_responses
  request_body: {}
expected_losses: []
expected_unsupported: []
risk_level: P1
```

#### 通过标准

- 每个协议至少具备最小样本集。
- 每个协议特有能力至少有一条专门样本。
- 每条样本都要写出 `expected_losses` 或 `expected_unsupported`，不能只写 happy path。

---

### 五、L3 Ingress 归一化核验

#### 目标

验证入口适配器设计是否真的能把不同协议安全地转成 RequestEnvelope，而不是只做字段搬运。

#### 必测问题

1. 是否能识别正确的 `source.protocol`？
2. 是否保留了 `original.body`？
3. 是否正确填充了 `normalized.model`？
4. 是否正确映射了系统提示？
5. 是否正确归一化了内容块类型？
6. 是否把不认识的块保留到 `UnknownPart`？
7. 是否把 provider 私有字段放进 `metadata.provider` 或 `raw`？
8. 是否记录了 ingress 阶段就能确定的不兼容风险？

#### 必测断言

| 断言 | 要求 |
| --- | --- |
| `source.protocol` | 必须正确 |
| `original.body` | 必须保留完整 |
| `normalized.input.messages` | 必须可重建核心语义 |
| `normalized.instructions` | 必须保留系统提示语义 |
| `normalized.tools` | 必须保留工具定义 |
| `raw / metadata.provider` | 必须保留暂不支持字段 |

#### 高风险点

| 风险点 | 说明 |
| --- | --- |
| Responses `input` 既可能是 string 又可能是 array | 归一化容易误压平 |
| Claude `system` 不在 messages 内 | 容易被误处理成普通 message |
| Gemini `parts` 是多态结构 | 容易只支持 text，漏掉 functionCall / inlineData |
| Deprecated `functions` / `function_call` | 如果不兼容旧字段，老客户端会断 |
| `background` / `previous_response_id` / `cachedContent` | 容易因为不参与 routing 被忽略 |

#### 通过标准

- 不能出现“入口一解析就丢信息”的情况。
- 对无法归一化的字段，必须保留 raw 或标记 unsupported。
- 不允许仅用 `messages` 结构替代所有协议语义。

---

### 六、L4 Egress 出口裁剪核验

#### 目标

验证目标协议生成规则是否完整，尤其是“同一份 Envelope 输出到不同目标协议时”是否有稳定规则。

#### 必测问题

1. 是否能根据 `target.protocol` 选择正确的出口适配器？
2. 是否有系统提示放置规则？
3. 是否有内容块降级规则？
4. 是否有工具定义转换规则？
5. 是否有 tool result 转换规则？
6. 是否能处理目标协议不支持的字段？
7. 是否有 Responses / Chat / Claude / Gemini 各自特有结构的生成规则？

#### 高风险转换对

开发前必须重点评审以下转换对：

| 转换对 | 风险说明 |
| --- | --- |
| Responses -> OpenAI Chat | `previous_response_id`、`reasoning`、MCP tool、include、background 无法直接表达 |
| OpenAI Chat -> Responses | messages 不能简单拼文本，tool calls 要重建 input 结构 |
| Claude -> OpenAI Chat | `system` 顶层、`tool_use` 块、thinking 块都需要重映射 |
| OpenAI Chat -> Claude | system message 必须提升到顶层，tool result 结构不同 |
| Gemini -> OpenAI Chat | `parts` 多态、functionCall、inlineData 会丢失风险 |
| OpenAI Chat -> Gemini | tools 要转 `functionDeclarations`，system 要转 `systemInstruction` |

#### 通过标准

- 每个高风险转换对都必须写出映射规则。
- 不支持的字段必须写出 compatibility 处理方式。
- 不允许出现“出口时再临时猜语义”的设计。

---

### 七、L5 Compatibility 风险核验

#### 目标

验证不兼容能力是否都有稳定的处理分支，避免后续开发中隐式丢字段。

#### 必测能力类别

| 类别 | 示例 |
| --- | --- |
| 会话延续 | `previous_response_id`、`cachedContent` |
| 后台执行 | `background` |
| 安全控制 | `safetySettings` |
| 推理控制 | `reasoning`、`thinking` |
| 工具扩展 | MCP、computer_use、web_search、file_search |
| 结构化输出 | `json_schema` |
| 多候选输出 | `n > 1` |
| 上下文截断 | `truncation` |
| 多模态 | image/audio/video/file |

#### 验证方式

对每种能力都要判断：

```text
是否原生支持
是否可降级
是否只能保留 raw
是否必须报错
```

并形成如下矩阵：

| 能力 | OpenAI Chat | Responses | Claude | Gemini | 处理策略 |
| --- | --- | --- | --- | --- | --- |
| previous_response_id | 否 | 是 | 否 | 否 | Chat/Claude/Gemini 下 drop 或 move_to_metadata |
| background | 否 | 是 | 否 | 否 | 非 Responses 目标下 strict error / lossy drop |
| MCP tool | 否 | 是 | 否 | 否 | 第一版保留 raw / unsupported |
| safetySettings | 否 | 否 | 否 | 是 | 非 Gemini 目标下 move_to_metadata 或 internal policy |

#### 通过标准

- 每项能力都必须有策略。
- 不允许出现“暂时先忽略”的未定义状态。
- 每项策略都要能映射到 `strict / lossy / best_effort`。

---

### 八、L6 Streaming / 生命周期核验

#### 目标

验证设计是否足以承载各协议的流式输出与响应状态变化。

#### 必测问题

1. 是否能表达 text delta？
2. 是否能表达 tool call delta？
3. 是否能表达 reasoning delta？
4. 是否能表达 usage event？
5. 是否能表达 terminal event？
6. 是否能表达 Responses 的后台状态变化？
7. 是否能表达 provider error 和 stream 中断？

#### 建议必备事件

```ts
type StreamEvent =
  | StreamStartEvent
  | StreamStatusEvent
  | StreamTextDeltaEvent
  | StreamToolCallDeltaEvent
  | StreamReasoningDeltaEvent
  | StreamUsageEvent
  | StreamEndEvent
  | StreamErrorEvent;
```

#### 额外高风险点

| 风险点 | 说明 |
| --- | --- |
| OpenAI Chat SSE 只强调 delta chunk | 容易忽略状态语义 |
| Responses 流中可能有 richer event taxonomy | 不能只当 text delta |
| Claude stream 有 content block 级事件 | 不能只做 token 文本流 |
| Gemini stream 可能是 JSON chunk 体系 | 需要内部统一事件层 |
| background response status | 不是普通 token stream |

#### 通过标准

- StreamEvent 设计必须覆盖文本、工具、reasoning、usage、状态、错误、结束。
- 不允许把后台任务状态伪装成普通文本事件。

---

### 九、开发前准入测试清单

进入开发阶段前，必须完成以下测试性评审并给出结论。

| 编号 | 检查项 | 结果要求 |
| --- | --- | --- |
| G1 | 官方协议核心字段是否已核验 | 必须完成 |
| G2 | 每协议最小样本集是否齐全 | 必须完成 |
| G3 | 高风险样本是否齐全 | 必须完成 |
| G4 | Ingress 是否能保留原始体 | 必须完成 |
| G5 | Egress 是否有明确映射规则 | 必须完成 |
| G6 | Compatibility 是否无未定义状态 | 必须完成 |
| G7 | Streaming 事件是否覆盖生命周期 | 必须完成 |
| G8 | 第一开发版支持边界是否明确 | 必须完成 |
| G9 | unsupported feature 是否有处理策略 | 必须完成 |
| G10 | 是否仍能快速回退到旧路径 | 必须完成 |

准入规则：

- 任一项未完成，不进入开发。
- 任一 P0 风险未关闭，不进入开发。
- 任一高风险转换对无映射规则，不进入开发。

---

### 十、建议的测试产物

虽然当前封版不开发，但建议把未来要产出的测试工件先定义清楚。

#### 1. 协议样本库

```text
docs/protocol-samples/
  openai-chat/*.json
  openai-responses/*.json
  claude-messages/*.json
  gemini-generate-content/*.json
  azure-openai/*.json
```

#### 2. 预期归一化结果

```text
docs/protocol-expected/
  normalized/*.yaml
  compatibility/*.yaml
  egress/*.yaml
```

#### 3. 高风险矩阵

```text
docs/protocol-risk-matrix.md
```

#### 4. 开发前评审记录

```text
docs/protocol-review-checklist.md
```

---

### 十一、最终结论

开发前验证的重点不是证明“设计看起来完整”，而是证明：

1. 设计没有遗漏主流协议的关键语义。
2. 设计能够解释不兼容，而不是隐藏不兼容。
3. 设计能为后续实现提供稳定、可测试、可回退的路径。

因此，在进入下一开发周期之前，必须先完成本章节定义的六层验证。

一句话总结：

> 先把风险点测试设计清楚，再开始开发；否则协议层很容易在 ingress、egress、streaming 或 compatibility 任一环出现“设计上觉得可以，实际一做就断”的问题。

---

## 特色功能融入论证与补充注意事项

本章节用于回答一个关键问题：

> API Switch 当前已有的特色功能，在未来接入 Envelope 协议层后，会受到什么影响？哪些方面会变好？哪些方面会变差？还有哪些额外风险需要提前控制？

结论先行：

> Envelope 方案本身不会天然削弱 API Switch 的特色能力；如果边界设计正确，它会把特色能力从“某个协议路径里的特殊逻辑”升级为“平台级能力”。
>
> 但如果边界失控，特色逻辑散落到各协议 adapter 中，或者 normalized 设计过重、过薄，都会反过来损害现有特色能力。

---

### 一、哪些特色功能会受影响

结合 API Switch 现有架构，未来会明显受到 Envelope 设计影响的特色能力主要包括：

- 模型池与 group 路由。
- 渠道选择与模型别名重写。
- fallback / failover。
- 熔断与冷却。
- 排序策略（custom / latest / fastest）。
- access key 与认证审计。
- usage log / audit log。
- 协议翻译中继。
- 上下游协议补偿。
- 调试能力与故障定位。

这些能力都不应该继续依附在 OpenAI Chat、Responses、Claude、Gemini 的私有字段结构上，而应转移到平台内核层。

---

### 二、融入后的正面影响

#### 1. 特色能力会从协议分支逻辑升级为平台能力

当前若按协议分别处理，很容易出现：

- OpenAI Chat 有一套增强逻辑。
- Responses 有另一套增强逻辑。
- Claude / Gemini 再各补一套特殊判断。

这样会导致每新增一个协议，都要复制一遍特色逻辑。

Envelope 方案下，特色能力可以统一落在：

```ts
internal: InternalContext
```

这样未来无论入口协议是什么，都先进入统一平台语义，再走同一套路由、策略、fallback、审计、调试链路。

正面价值：

- 降低协议分支重复逻辑。
- 保证不同协议入口下的平台行为一致。
- 让路由、熔断、日志等能力不再依赖某个外部 JSON 结构。

#### 2. 模型路由与渠道选择会更稳定

不同协议下，模型信息可能来自：

- `body.model`
- `deployment`
- path 参数
- `input` 间接语义

如果没有统一中间态，路由层就会不断依赖协议细节。

Envelope 后，可以统一为：

```ts
internal.routing.requestedModel
internal.routing.normalizedModel
internal.routing.selectedModel
internal.routing.selectedEntryId
internal.routing.selectedChannelId
internal.routing.selectedApiType
```

这样模型池、分组、渠道选择就只依赖内部上下文，不依赖外部协议形态。

#### 3. 协议互通会更可解释

API Switch 的中间层价值之一，不只是“转得过去”，而是“知道怎么转、丢了什么、为什么丢”。

Envelope 里的：

```ts
compatibility.losses
compatibility.warnings
compatibility.unsupported
```

可以把协议损耗显式化。例如：

- Responses 的 `previous_response_id` 转 Chat 被丢弃。
- Gemini 的 `safetySettings` 转 OpenAI 被降级。
- Claude 的 `thinking` 转 Chat 被抑制。
- MCP tool 在非 Responses 目标下被标记 unsupported。

这会强化你们的特色：

- 用户更容易理解协议行为。
- 调试更容易。
- 日志更有价值。
- 后续 UI 甚至可以展示转换损耗摘要。

#### 4. 为未来扩协议留出稳定接口

如果未来增加更多协议或兼容 provider，仅用两两互转会出现组合爆炸。

Envelope 架构把复杂度收敛为：

```text
新协议 -> ingress adapter
Envelope -> 新协议 egress adapter
```

这样你们自己的特色能力不用为新协议重写。

#### 5. 有利于灰度演进和回退

程序已封版，未来开发一定要稳。

Envelope 可以先以 shadow 模式验证：

```text
旧路径继续服务
新协议层只做 shadow parse / compare / log
```

这样可以：

- 不影响现有用户流量。
- 提前收集设计与真实请求的偏差。
- 做新旧输出比对。
- 保留快速回退能力。

---

### 三、潜在负面影响

#### 1. 如果 normalized 设计过重，会变成超级中间协议

坏处：

- 类型膨胀。
- 协议更新时核心结构频繁变动。
- 开发者不敢修改。
- 每个协议私有语义都被硬塞进 normalized。

控制原则：

> Normalized 只放平台必须理解的公共字段；协议私有细节继续放 `original`、`raw`、`metadata.provider`。

#### 2. 如果特色逻辑被塞进 adapter，会反向耦合

最危险的实现方式是：

- 在 OpenAI adapter 里做路由。
- 在 Claude adapter 里做 fallback。
- 在 Gemini adapter 里做 usage log。
- 在 Responses adapter 里做熔断。

这样每加一个协议，就要复制一套特色能力。

稳定边界必须是：

```text
Ingress Adapter：只解析来源协议
Internal Layer：处理 API Switch 特色能力
Egress Adapter：只生成目标协议
```

#### 3. 现有特色逻辑中的隐含协议假设会暴露出来

例如某些现有逻辑可能默认：

- 请求一定有 `messages`。
- system prompt 一定是第一条 message。
- 模型一定在 body.model。
- tool call 一定是 OpenAI Chat 结构。
- stream 一定是 OpenAI SSE chunk。

引入 Envelope 后，这些假设会失效，需要迁移为基于：

```ts
normalized
internal
capabilities
compatibility
```

这会增加设计和后续实现成本。

#### 4. 日志和审计结构会更复杂

未来日志需要记录的不只是：

- requested model
- selected model
- channel id
- latency
- status

还可能需要：

- source protocol
- target protocol
- translation mode
- compatibility losses
- stream lifecycle
- response status

这会增加日志设计复杂度，但也是更强审计能力的代价。

#### 5. 流式协议统一是高风险区

不同协议流式模型差异很大：

- OpenAI Chat：delta chunk
- Responses：typed event stream
- Claude：content block 事件
- Gemini：JSON chunk / streamGenerateContent

如果过早把它们都压成“纯文本流”，会损害：

- tool call delta
- reasoning delta
- usage event
- finish reason
- background status
- stream error

因此 streaming 必须作为后期能力处理，而不是第一版强行统一。

---

### 四、对各类特色功能的具体影响判断

| 特色功能 | 正面影响 | 风险点 | 设计要求 |
| --- | --- | --- | --- |
| 模型路由 | 入口协议差异被屏蔽，路由更统一 | Azure deployment、Gemini path model 可能被误归一化 | 路由只读 `internal.routing` |
| group / pool | 所有协议共享同一套选择逻辑 | adapter 若擅自改 model，会绕过 group | group 选择必须在 internal 层 |
| fallback | 可按 capability 做更聪明降级 | stream 中途 fallback 极难 | fallback 决策不放在 adapter |
| 熔断 / 冷却 | 可以统一按 selected entry 记录 | 协议翻译错误不应记成 provider 失败 | 区分 translation error 与 provider error |
| 排序策略 | 与协议解耦，可继续复用 | 协议差异可能影响“测速/最新”语义展示 | sort mode 放 `internal.policy` |
| access key | 与协议无关，天然可复用 | 不同协议 header auth 不能覆盖内部认证 | auth 统一放 internal/auth |
| usage / audit | 可以更强地记录 source/target/losses | 日志字段膨胀 | 结构化日志与字段分层 |
| 协议翻译中继 | 入口协议标注后翻译依据更充分 | 归一化过度压平会丢语义 | 保留 original/raw |
| 调试能力 | 可展示完整转换链路 | 可能暴露敏感 body | 增加敏感字段脱敏策略 |

---

### 五、必须坚持的边界

#### Ingress Adapter 只做什么

只做：

- 协议识别。
- 原始请求解析。
- `source.protocol` 填充。
- `original` 保留。
- `normalized` 填充。
- raw / metadata.provider 保留。

不做：

- 路由。
- fallback。
- usage log。
- 熔断。
- 数据库访问。
- 渠道选择。

#### Internal Layer 负责什么

只在 internal 层处理平台特色：

```ts
internal.routing
internal.policy
internal.features
internal.audit
internal.debug
```

包括：

- selected entry / channel / group。
- sort mode。
- compatibility mode。
- fallback 策略。
- feature flags。
- audit labels。
- 调试 trace。

#### Egress Adapter 只做什么

只做：

- 根据 `target.protocol` 生成目标请求。
- 根据 target capability 裁剪字段。
- 记录 compatibility loss。
- 生成 provider request。

不做：

- 再次路由。
- 改变 selected channel。
- 写数据库。
- 决定 fallback。

---

### 六、额外需要补充注意的事项

除了前文已有内容，以下事项也建议写入稳定计划。

#### 1. 特色能力的“协议无关输入面”要提前定义

以后所有平台特色能力都应尽量依赖统一输入面，而不是直接读外部协议字段。

建议未来定义：

```ts
interface InternalFeatureInput {
  model?: string;
  messages?: NormalizedMessage[];
  tools?: NormalizedTool[];
  stream?: boolean;
  sourceProtocol: Protocol;
  targetProtocol?: Protocol;
  compatibilityMode: CompatibilityMode;
}
```

后续无论是模型路由、fallback、审计、风控、翻译增强，都优先基于这层做逻辑。

#### 2. translation error 与 provider error 必须分离

这是一个很重要但容易遗漏的点。

未来日志和熔断中必须区分：

- 翻译层错误：协议不兼容、字段缺失、adapter bug。
- provider 错误：上游 4xx / 5xx / 超时 / 限流。

如果不分离，会导致：

- 协议 bug 被误判为渠道不稳定。
- 熔断打错对象。
- 延迟统计失真。
- 失败回退策略错误。

建议未来错误结构中固定字段：

```ts
error.source = "translation" | "provider" | "internal"
```

#### 3. 敏感字段脱敏策略要提前考虑

因为 Envelope 会保留 `original.body`，这意味着：

- API key
- 用户输入
- tools 参数
- provider metadata
- 可能的文件内容或 URL

都可能进入调试与日志链路。

所以未来必须补：

- 调试日志脱敏。
- raw body 采样开关。
- 敏感字段清单。
- 按环境控制 debug 可见性。

#### 4. 不要把“能保留 raw”误当成“已经支持”

这是评审时非常容易犯的错。

必须区分三种状态：

| 状态 | 含义 |
| --- | --- |
| supported | 设计和实现都明确支持并可稳定转换 |
| preserved_only | 只保留 raw，尚不能稳定跨协议转换 |
| unsupported | 明确不支持，必须报错或降级 |

未来设计评审和实现验收都应按这三类标注，不要因为字段被保留了就说“已兼容”。

#### 5. 第一版不要追求跨所有协议完全等价

因为部分协议能力本质上不等价：

- Responses `background`
- Responses MCP tool
- Claude thinking
- Gemini safety settings
- cachedContent / previous_response_id

第一版目标应是：

> 保证核心能力稳定互通，复杂能力清楚标注损耗或 unsupported。

而不是追求“所有协议 100% 对等”。

---

### 七、建议写入稳定计划的最终补充结论

在封版稳定计划中，应补上以下结论：

1. Envelope 方案对 API Switch 特色功能总体是增强，不是削弱。
2. 特色功能必须沉淀到 `InternalContext`，不能散落到各协议 adapter。
3. 所有平台能力必须逐步摆脱对 OpenAI Chat 结构的隐性依赖。
4. translation error / provider error / internal error 必须明确区分。
5. raw 保留不等于已支持，必须区分 `supported` / `preserved_only` / `unsupported`。
6. streaming 是特色能力融合中的最高风险点，必须后置实施。
7. 调试能力增强的同时，必须补脱敏和安全策略。

一句话总结：

> 这套方案最大的价值，不是“多支持了几个协议”，而是让 API Switch 的特色能力脱离单一协议结构，真正成为平台级能力；最大的风险，也不是协议字段多少，而是边界没守住导致平台能力重新散落回各协议分支里。

---

## 行为补偿机制设计

本章节用于固化 API Switch 的一个重要特色能力：

> 当目标协议或目标上游不支持某些来源协议能力时，除了做字段级降级，还可以通过提示词注入、系统指令补充、运行时能力引导等方式，对下游模型行为进行纠正或补偿。

这个机制不是普通协议字段映射，而是平台级“行为补偿”能力。

---

### 一、为什么要单独设计行为补偿

协议转换里有两类不兼容：

#### 第一类：字段不兼容，但语义可近似补偿

例如：

- Responses 原生工具类型下游不支持。
- 严格结构化输出下游不支持，但仍可提示“只输出 JSON”。
- 某些推理开关下游不支持，但可提示“仅输出结果，不展示中间推理”。
- 某些多模态能力缺失，但可退化为文本模式回答。

这类问题不一定要直接报错，可以通过行为引导让模型尽量采取可替代路径。

#### 第二类：系统能力不兼容，不能靠提示词伪装支持

例如：

- `previous_response_id`
- `background`
- `cachedContent`
- provider-native conversation state
- provider-side safety enforcement
- 真正的 MCP server 远程执行

这类能力不属于“模型行为问题”，而属于“平台或协议能力缺失”，不能仅靠提示词声称已支持。

因此必须把“行为补偿”与“字段级兼容”和“系统能力不支持”区分开。

---

### 二、当前代码中的现有雏形

当前项目里，这个能力已经有一个明确雏形，存在于 Responses -> Chat fallback 路径中。

相关位置：

- `D:\Work\api-switch\src-tauri\src\proxy\protocol\responses.rs:218`
- `D:\Work\api-switch\src-tauri\src\proxy\protocol\responses.rs:238`
- `D:\Work\api-switch\src-tauri\src\proxy\protocol\responses.rs:251`
- `D:\Work\api-switch\src-tauri\src\proxy\protocol\responses.rs:270`
- `D:\Work\api-switch\src-tauri\src\proxy\protocol\responses.rs:2100`

现有实现做了三件事：

1. 识别 Responses request 中哪些工具属于 Chat 上游不支持的原生工具类型。
2. 对不支持的工具类型，不直接透传给 Chat 上游。
3. 同时给 system prompt 注入一段“行为纠正提示”，告诉下游模型：
   - 当前环境没有这些 Responses native tool。
   - 请改用 shell、HTTP、scripts、browser、filesystem、DB 等当前可用运行时能力完成任务。
   - 结果必须来自真实执行，不得编造。

这说明当前系统已经不是简单“丢字段”，而是具备：

> 字段级降级 + 行为级补偿

这正是 API Switch 中间层特色能力的一部分，应该正式写进架构设计，而不是继续留在 adapter 的局部逻辑中。

---

### 三、行为补偿的正式定义

建议在设计中增加如下概念：

```ts
interface BehavioralCompensation {
  reason: string;
  sourceProtocol: Protocol;
  targetProtocol: Protocol;
  kind:
    | "tool_unavailable"
    | "stateful_conversation_unavailable"
    | "background_execution_unavailable"
    | "reasoning_capability_degraded"
    | "safety_control_unavailable"
    | "structured_output_not_strict"
    | "multimodal_capability_degraded"
    | "provider_runtime_capability_gap";
  strategy: "inject_system_prompt" | "append_instruction" | "metadata_only";
  prompt?: string;
  severity: "info" | "warning" | "error";
  raw?: unknown;
}
```

建议挂载位置：

```ts
interface CompatibilityState {
  mode: "strict" | "lossy" | "best_effort";
  losses: CompatibilityLoss[];
  warnings: CompatibilityWarning[];
  unsupported: UnsupportedFeature[];
  compensations?: BehavioralCompensation[];
}
```

也可以在 internal policy 中保留运行时决策痕迹：

```ts
internal.policy.behaviorCompensations?: BehavioralCompensation[];
```

推荐做法：

- `CompatibilityState.compensations`：对外描述这次协议转换做了哪些行为补偿。
- `internal.policy.behaviorCompensations`：对内记录平台为什么这样做、由哪个规则触发。

---

### 四、行为补偿与兼容模式的关系

行为补偿不是默认总启用，而应受兼容模式控制。

#### strict

```text
发现目标协议不支持某能力 -> 直接报错
```

strict 下通常不应自动注入补偿 prompt，因为 strict 的目标是避免语义漂移。

#### lossy

```text
优先字段降级/丢弃 -> 记录损耗 -> 仅在明确安全时允许补偿
```

lossy 下可对少量高价值场景开启行为补偿，但必须记录：

- 原能力是什么
- 为什么降级
- 注入了什么补偿
- 可能存在哪些语义变化

#### best_effort

```text
允许行为补偿作为主要策略之一
```

best_effort 是行为补偿最合适的模式。

因此建议：

| 模式 | 是否允许行为补偿 |
| --- | --- |
| `strict` | 默认不允许 |
| `lossy` | 仅允许白名单场景 |
| `best_effort` | 明确允许 |

---

### 五、适合行为补偿的场景

以下场景适合通过提示词或系统指令进行补偿。

#### 1. 原生工具不可用，但运行时存在可替代能力

典型例子：

- Responses native tools 在 Chat 上游不可用。
- 目标上游不支持某内置工具，但平台运行时仍有 shell、HTTP、filesystem、browser 等可替代能力。

这类场景适合注入补偿 prompt，引导模型改用当前运行时能力。

#### 2. 结构化输出无法严格保证，但可以要求输出格式

例如：

- 来源协议要求 strict `json_schema`
- 目标协议只支持普通文本或弱 JSON 模式

可以注入：

- 只输出 JSON
- 严格遵循以下字段结构
- 不输出额外解释文本

但必须标记为：

```text
structured_output_not_strict
```

#### 3. 推理能力或展示方式降级

例如：

- 来源协议支持 reasoning / thinking
- 目标协议不支持可见推理块

可以注入行为约束：

- 只给最终答案
- 不展示中间推理过程
- 若无法确定，则明确说明不确定性

#### 4. 多模态退化为文本模式

例如：

- 目标协议不支持图像/文件能力
- 但平台决定允许文本级退化

可注入：

- 如果无法访问原图像/文件，请明确说明限制
- 仅基于可用文本上下文回答

---

### 六、不适合行为补偿的场景

以下能力不能靠 prompt 假装支持，必须按 unsupported / loss 处理。

#### 1. provider-side state 能力

包括：

- `previous_response_id`
- `cachedContent`
- 服务器端对话状态延续

prompt 不能替代真实的服务端会话状态。

#### 2. 真正的后台任务能力

包括：

- `background`
- polling-based lifecycle

prompt 不能让一个同步接口变成异步后台任务系统。

#### 3. provider-native safety enforcement

`safetySettings` 等字段往往是上游平台侧控制，不是模型自然语言行为可完全替代的能力。

#### 4. 远程 MCP server 真调用

如果目标协议和运行时没有 MCP 执行能力，就不能仅通过 prompt 说“请当成有 MCP”。

#### 5. 严格 schema guarantee

如果目标协议本身不提供强 schema enforcement，prompt 只能做弱约束，不能宣称“等价支持 strict schema”。

---

### 七、行为补偿的触发流程

建议未来统一按以下流程处理：

```mermaid
flowchart TD
  A["Envelope + target capability"] --> B["发现来源能力与目标能力不匹配"]
  B --> C{"能否安全降级?"}
  C -->|否| D["unsupported / strict error"]
  C -->|是| E{"是否属于可行为补偿白名单?"}
  E -->|否| F["lossy drop / metadata only"]
  E -->|是| G["生成 BehavioralCompensation"]
  G --> H["注入 system prompt / instruction"]
  H --> I["记录 compatibility.compensations"]
```

关键原则：

- 先判断能力不匹配。
- 再判断是否属于允许行为补偿的白名单。
- 不在白名单内的，不允许随意注入 prompt 假装支持。

---

### 八、建议的白名单策略

建议未来只对以下类型开启行为补偿白名单：

| kind | 默认策略 |
| --- | --- |
| `tool_unavailable` | 允许 |
| `structured_output_not_strict` | 允许 |
| `reasoning_capability_degraded` | 允许 |
| `multimodal_capability_degraded` | 谨慎允许 |
| `provider_runtime_capability_gap` | 谨慎允许 |
| `stateful_conversation_unavailable` | 不允许 |
| `background_execution_unavailable` | 不允许 |
| `safety_control_unavailable` | 默认不允许 |

这样可以避免行为补偿被滥用成“什么都靠 prompt 糊过去”。

---

### 九、提示词注入的边界要求

行为补偿如果最终采用 `inject_system_prompt`，必须满足以下约束。

#### 1. 不能覆盖用户原始 system/instructions

只能：

- 追加到已有 system 后面
- 或作为新的低优先级补偿 instruction 插入

不能直接替换原始用户意图。

#### 2. 必须可追踪

需要记录：

- 注入原因
- 注入策略
- 注入文本摘要
- 所属 request id

否则后续难以解释模型为什么表现出某种行为。

#### 3. 必须可禁用

未来至少要支持：

- 全局关闭行为补偿
- 按兼容模式关闭
- 按 target protocol 关闭
- 按 feature kind 关闭

#### 4. 必须脱敏

注入日志或调试信息时，不应把用户敏感输入拼接进通用日志，避免 raw prompt 泄漏。

---

### 十、与现有 responses.rs 实现的映射关系

当前实现可以视为行为补偿机制的第一条已存在规则：

| 项目 | 当前实现 |
| --- | --- |
| 触发源 | Responses request 含 Hosted / native tool |
| 目标 | Chat 上游不支持该 tool 类型 |
| 行为 | 跳过不支持工具 + 注入 system prompt |
| 兼容语义 | best-effort behavioral compensation |
| 记录方式 | 当前主要是 `warn!` 日志 |
| 未来升级方向 | 进入 `CompatibilityState.compensations` |

建议未来把当前实现抽象为一条正式规则：

```ts
{
  kind: "tool_unavailable",
  sourceProtocol: "openai_responses",
  targetProtocol: "openai_chat",
  strategy: "inject_system_prompt",
  severity: "warning"
}
```

也就是说，当前 `responses.rs` 的局部 hardcode 不应被删除，而应被提升为平台正式能力，并逐步迁移到统一 compatibility / policy 层。

---

### 十一、测试与评审要求

行为补偿一旦纳入正式设计，未来开发前必须新增验证项。

#### 样本测试新增要求

至少补充以下样本：

| 样本 | 目的 |
| --- | --- |
| responses_tool_unavailable_prompt_compensation | 验证不支持工具时会生成行为补偿 |
| structured_output_degraded_to_prompt | 验证 strict schema 降级为 prompt 约束 |
| reasoning_degraded_with_instruction | 验证 reasoning 能力降级时的行为提示 |
| background_unavailable_should_error | 验证不该补偿的能力必须报错 |
| previous_response_id_should_not_fake_support | 验证状态能力不能靠 prompt 伪装 |

#### 开发前评审新增问题

1. 本次转换是否使用了行为补偿？
2. 该补偿是否属于白名单场景？
3. 该补偿是否会误导系统“已原生支持”？
4. 该补偿是否可关闭？
5. 该补偿是否已记录到 compatibility / debug trace？

---

### 十二、最终结论

API Switch 现有的“当目标不支持时，用提示词纠正下游行为”的能力：

- 不是偶然实现，而是一个非常有价值的中间层特色。
- 当前已经在 Responses -> Chat fallback 中有明确雏形。
- 完全应该纳入 Envelope 总体设计。
- 但必须被定义为“行为补偿机制”，而不是普通字段兼容。
- 必须和 `strict / lossy / best_effort`、`supported / preserved_only / unsupported`、compatibility losses 一起管理。

一句话总结：

> 当协议能力无法等价映射时，API Switch 不只是做“字段降级”，还可以做“行为补偿”；这正是中间层平台价值的一部分，但必须作为受控能力使用，不能变成掩盖真实不兼容的手段。

---

## 支持状态定义

为了避免“文档写了”被误解为“代码已经支持”，本设计统一使用以下状态来描述每个协议字段或能力的当前定位。

| 状态 | 含义 |
| --- | --- |
| `implemented` | 已有实现，且已有明确代码路径支撑 |
| `designed_supported` | 设计已完整定义，目标是后续实现为稳定支持 |
| `designed_partial` | 设计已定义，但只支持部分协议或部分形态 |
| `preserved_only` | 设计已考虑，但当前只要求保留 raw / metadata / notes，不承诺跨协议稳定转换 |
| `unsupported` | 设计已考虑，但明确不支持，必须报错、降级或记录损耗 |

### 判定规则

#### 1. `implemented`

只有在当前代码库中已经存在可定位实现路径时，才能标记为 `implemented`。

例如：

- Responses -> Chat 路径中对不支持 Hosted / native tools 的提示词补偿，已在 `D:\Work\api-switch\src-tauri\src\proxy\protocol\responses.rs:218` 等位置有实现雏形，因此可标记为局部 `implemented`。

#### 2. `designed_supported`

表示：

- 协议字段或能力已经在设计中有明确承载位置。
- 有清楚的 ingress / egress / compatibility 处理思路。
- 后续版本计划作为正式支持能力实现。

#### 3. `designed_partial`

表示：

- 设计层面已经定义，但并非所有协议对都能稳定支持。
- 或仅支持该能力的一部分子集。

典型例子：

- tool conversion 里 function tools 可支持，但 provider-native tools 只能部分支持。
- structured outputs 可部分降级，但不能保证所有协议对都严格等价。

#### 4. `preserved_only`

表示：

- 该能力已被设计捕获。
- 当前阶段只保证：不在入口阶段丢失。
- 不承诺在第一版里实现稳定互转。

这类能力通常保存在：

- `original.body`
- `raw`
- `metadata.provider`
- `UnknownPart`
- `CompatibilityState.unsupported`

#### 5. `unsupported`

表示：

- 设计上已经明确知道该能力与当前目标协议或当前实现阶段不兼容。
- 不允许伪装成“已兼容”。
- 必须进入：
  - 报错
  - drop
  - downgrade
  - move_to_metadata
  - 或明确 unsupported 提示

### 重要说明

必须严格区分以下四件事：

1. 协议中存在这个字段。
2. 设计中已经考虑这个字段。
3. 当前版本已经实现这个字段。
4. 当前版本已经稳定支持这个字段的跨协议互转。

只有第 3 和第 4 同时满足时，才可认为用户侧“真的支持”。

一句话原则：

> 写进文档不等于已经支持；保留 raw 也不等于已经兼容。

---

## 协议能力状态总表

本表用于把“协议字段存在性”“设计承载方式”“第一版计划状态”“实现注意事项”集中整理，避免信息散落在各章节中。

说明：

- `设计承载` 表示该能力在文档中的主要落点。
- `第一版状态` 采用上一节定义的状态。
- `处理方式` 表示推荐的默认处理策略，而非当前代码已全部实现。

| 协议 | 能力/字段 | 设计承载 | 处理方式 | 第一版状态 | 备注 |
| --- | --- | --- | --- | --- | --- |
| OpenAI Chat | `messages` | `normalized.input.messages` | 直接支持 | `designed_supported` | 核心基础能力 |
| OpenAI Chat | system/developer messages | `normalized.instructions` + messages | 提升或回落 | `designed_supported` | system 需与其他协议位置对齐 |
| OpenAI Chat | `tools` function | `normalized.tools` | 直接支持 | `designed_supported` | 第一版重点支持 |
| OpenAI Chat | `tool_choice` | `normalized.toolChoice` | 直接支持 | `designed_supported` | 与 Claude/Gemini 需做模式映射 |
| OpenAI Chat | deprecated `functions` / `function_call` | `normalized.tools` / `normalized.toolChoice` | 兼容入口映射 | `designed_supported` | 为老客户端保留 |
| OpenAI Chat | `response_format.json_object` | `normalized.responseFormat` | 直接支持或降级 | `designed_supported` | 文本模型需额外提示 |
| OpenAI Chat | `response_format.json_schema` | `normalized.responseFormat` | 支持或降级为行为补偿 | `designed_partial` | 跨协议严格保证不足 |
| OpenAI Chat | `stream` | `normalized.stream` + `StreamEvent` | 直接支持 | `designed_supported` | 第一版只要求基础流语义 |
| OpenAI Chat | `n` | `generation.n` | 不支持时降为 1 | `preserved_only` | 第一版不建议完整互转 |
| OpenAI Chat | `user` | `metadata.client.user` | 保留 | `preserved_only` | 非核心转换语义 |
| OpenAI Chat | `logit_bias` | `generation.logitBias` | 保留或 unsupported | `preserved_only` | 第一版不建议强支持 |
| OpenAI Chat | `reasoning` | `normalized.reasoning` | 保留或降级 | `designed_partial` | 依目标模型能力而定 |
| OpenAI Responses | `input` | `normalized.input.messages` + `original.body` | 直接支持 | `designed_supported` | 与 Chat messages 不完全等价 |
| OpenAI Responses | `instructions` | `normalized.instructions` | 直接支持 | `designed_supported` | 第一版重点支持 |
| OpenAI Responses | `previous_response_id` | `conversation.previousResponseId` | 非 Responses 目标下 drop / metadata | `preserved_only` | 不能靠 prompt 伪装 |
| OpenAI Responses | `store` | `conversation.store` | 保留 | `preserved_only` | 第一版不做完整语义支持 |
| OpenAI Responses | `background` | `execution.background` | 非 Responses 目标 strict error | `unsupported` | 第一版不做代理后台任务 |
| OpenAI Responses | `truncation` | `conversation.truncation` | 保留 / metadata | `preserved_only` | 第一版不做稳定互转 |
| OpenAI Responses | `include` | `output.include` | 保留或 drop | `preserved_only` | 第一版可不转换 |
| OpenAI Responses | native hosted tools / MCP tools | `tools` + `BehavioralCompensation` | function 转换，其余补偿或 unsupported | `designed_partial` | 现有 `responses.rs` 已有局部实现 |
| OpenAI Responses | `parallel_tool_calls` | `toolOptions.parallelToolCalls` | 保留或降级 | `preserved_only` | 第一版不做完整并行语义 |
| OpenAI Responses | 状态生命周期 | `NormalizedResponse.status` | 保留 | `preserved_only` | queued/in_progress/completed 等 |
| OpenAI Responses | typed streaming events | `StreamEvent` | 统一流事件 | `designed_partial` | 第一版不建议全互转 |
| Azure OpenAI | deployment path | `target.deployment` | 直接支持 | `designed_supported` | Azure 核心差异 |
| Azure OpenAI | `api-version` | `target` / adapter config | 适配器处理 | `designed_supported` | 不进入 normalized 核心语义 |
| Azure OpenAI | Chat request/response | Chat 兼容层 | 直接支持 | `designed_supported` | 与 OpenAI Chat 近似 |
| Azure OpenAI | Responses background / MCP | Responses + execution/tools | 保留或 unsupported | `preserved_only` | 依 Azure 官方演进 |
| Claude Messages | top-level `system` | `normalized.instructions` | 直接支持 | `designed_supported` | 必须避免误当普通 message |
| Claude Messages | `messages` | `normalized.input.messages` | 直接支持 | `designed_supported` | 核心基础能力 |
| Claude Messages | `tool_use` | `ToolCallPart` / tools | 映射为 function tool calls | `designed_partial` | 第一版可先做基础版 |
| Claude Messages | `tool_result` | `ToolResultPart` | 直接映射 | `designed_partial` | 结构不同于 Chat |
| Claude Messages | `thinking` | `normalized.reasoning` / `ReasoningPart.raw` | 保留或降级 | `preserved_only` | 第一版不做完整互转 |
| Claude Messages | `stop_sequences` | `generation.stop` | 直接映射 | `designed_supported` | 需明确入口处理 |
| Claude Messages | metadata/user_id | `metadata.provider` | 保留 | `preserved_only` | 非核心互转语义 |
| Gemini | `contents` / `parts` | `normalized.input.messages` | 直接支持 | `designed_supported` | 核心基础能力 |
| Gemini | `systemInstruction` | `normalized.instructions` | 直接支持 | `designed_supported` | 顶层系统提示 |
| Gemini | `functionDeclarations` | `normalized.tools` | 映射为 function tools | `designed_partial` | 第一版可支持基础版 |
| Gemini | `toolConfig` | `normalized.toolChoice.raw` | 保留或部分转换 | `preserved_only` | 第一版可不完整实现 |
| Gemini | `safetySettings` | `safety` | 非 Gemini 目标下 move_to_metadata / internal | `preserved_only` | 不能宣称等价支持 |
| Gemini | `cachedContent` | `conversation.cachedContent` | 保留 | `preserved_only` | 不能靠 prompt 伪装 |
| Gemini | inlineData / fileData | `ImagePart` / `FilePart` / raw | 保留或降级 | `designed_partial` | 第一版多模态仅建议基础支持 |
| 跨协议共性 | behavior compensation | `CompatibilityState.compensations` | 白名单启用 | `designed_supported` | 现有 Responses -> Chat 已有雏形 |
| 跨协议共性 | compatibility losses | `CompatibilityState.losses` | 全程记录 | `designed_supported` | 核心平台能力 |
| 跨协议共性 | provider private fields | `original.body` / `metadata.provider` / `raw` | 保留 | `designed_supported` | 入口不提前丢失 |
| 跨协议共性 | translation error vs provider error | `NormalizedError` / internal log | 必须区分 | `designed_supported` | 防止熔断和日志误判 |
| 跨协议共性 | background task polling | `InternalResponseContext.poll` | 第一版不实现 | `unsupported` | 文档已定义，暂不开发 |
| 跨协议共性 | full streaming lifecycle parity | `StreamEvent` | 分阶段实现 | `designed_partial` | 第一版不追求完全等价 |

### 总表使用原则

后续评审和开发时，必须按以下顺序判断：

1. 该能力协议中是否真实存在。
2. 文档中是否已有设计承载位置。
3. 该能力属于 `implemented`、`designed_supported`、`designed_partial`、`preserved_only` 还是 `unsupported`。
4. 第一版是否真的要实现。
5. 若不实现，是保留、降级、行为补偿还是报错。

一句话总结：

> 这张总表的目的，不是让第一版什么都做，而是保证“所有重要协议能力都已经被看见、被分类、被安置”。

