import type { ImageCount, ImageEndpoint, ImageReference, ImageResult, ImageSize } from "./types";

export interface BuildImageRequestParams {
  endpoint: ImageEndpoint;
  model: string;
  prompt?: string;
  size: ImageSize;
  count: ImageCount;
  referenceImage?: ImageReference;
}

export interface ImageRequestPayload {
  endpoint: ImageEndpoint;
  path: string;
  init: RequestInit;
}

export function buildImageRequest(params: BuildImageRequestParams): ImageRequestPayload {
  const path = `/v1/images/${params.endpoint}`;
  if (params.endpoint === "generations") {
    return {
      endpoint: params.endpoint,
      path,
      init: {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          model: params.model,
          prompt: params.prompt || "",
          size: params.size,
          n: params.count,
        }),
      },
    };
  }

  const form = new FormData();
  form.append("model", params.model);
  if (params.endpoint === "edits") form.append("prompt", params.prompt || "");
  form.append("size", params.size);
  form.append("n", String(params.count));
  if (params.referenceImage) {
    form.append("image", params.referenceImage.blob, params.referenceImage.name || "image.png");
  }
  return { endpoint: params.endpoint, path, init: { method: "POST", body: form } };
}

export function buildProxyBaseUrl(proxyStatus?: { running: boolean; address?: string; port?: number } | null, settings?: { listen_port?: number } | null) {
  const port = proxyStatus?.port || settings?.listen_port;
  if (!port) return "";
  const pageHost = window.location.hostname || "127.0.0.1";
  const rawAddress = proxyStatus?.address || pageHost;
  const host = rawAddress === "0.0.0.0" || rawAddress === "::" ? pageHost : rawAddress;
  const protocol = window.location.protocol === "https:" ? "https:" : "http:";
  return `${protocol}//${host}:${port}`;
}

export async function callImagesEndpoint(proxyBaseUrl: string, payload: ImageRequestPayload) {
  const response = await fetch(`${proxyBaseUrl}${payload.path}`, payload.init);
  const contentType = response.headers.get("content-type") || "";
  const text = await response.text();
  if (!response.ok) {
    const preview = text.replace(/\s+/g, " ").slice(0, 240);
    throw new Error(preview || `HTTP ${response.status}`);
  }
  if (!text.trim()) return [];
  let json: unknown;
  try {
    json = JSON.parse(text);
  } catch {
    throw new Error("上游返回了非标准图片结果");
  }
  const data = (json as { data?: unknown }).data;
  if (!Array.isArray(data) || data.length === 0) throw new Error("上游未返回可显示的图片");
  return data.map((item, index) => normalizeImageResult(item, index, contentType));
}

function normalizeImageResult(item: unknown, index: number, contentType: string): Omit<ImageResult, "id" | "recordId" | "index"> {
  const object = item && typeof item === "object" ? (item as Record<string, unknown>) : {};
  const b64Json = typeof object.b64_json === "string" ? object.b64_json : undefined;
  const url = typeof object.url === "string" ? object.url : undefined;
  if (b64Json) {
    const mime = contentType.includes("json") ? "image/png" : contentType || "image/png";
    const binary = atob(b64Json);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
    return { b64Json, objectUrl: URL.createObjectURL(new Blob([bytes], { type: mime })), mime };
  }
  if (url) return { url };
  throw new Error(`第 ${index + 1} 张图片缺少 url 或 b64_json`);
}
