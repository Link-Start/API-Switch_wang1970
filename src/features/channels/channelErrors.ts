import { useMemo } from 'react';
import type { ChannelOperationHttpError } from './types';

function withDebugInfo(title: string, description: string, message: string): string {
  return `${title}：${description}\n\n调试信息：\n${message}`;
}

export function getChannelErrorMessage(error: unknown, fallback: string): string {
  if (!error || !(error instanceof Error)) {
    return fallback;
  }

  const operationError = error as ChannelOperationHttpError;
  const message = operationError.error?.message || operationError.message || fallback;

  switch (operationError.kind) {
    case 'auth':
    case 'rate_limited':
      return withDebugInfo(
        '认证失败或账号不可用',
        '请检查 API Key 是否正确、是否过期，账号或组织是否可用，以及当前账号是否有权限访问该服务商',
        message,
      );
    case 'timeout':
    case 'network':
    case 'invalid_url':
      return withDebugInfo(
        '无法连接服务商',
        '请检查网络、代理、防火墙或 Base URL 是否可以访问',
        message,
      );
    case 'unsupported_provider':
    case 'empty_model_list':
    case 'endpoint_correction_failed':
      return withDebugInfo(
        '无法获取模型列表',
        '可能是 API 类型不匹配、Base URL 路径不正确，或该服务商不支持自动获取模型。你可以手动添加模型',
        message,
      );
    default:
      return withDebugInfo('无法获取模型列表', '请根据调试信息检查渠道配置，或手动添加模型', message);
  }
}

export function useChannelModelText(channel: { selected_models?: string[] } | null | undefined) {
  return useMemo(() => channel?.selected_models?.join(', ') ?? '', [channel?.selected_models]);
}

/** 测速失败时的展示信息：列表标签 + 悬浮完整调试信息 */
export interface TestFailureInfo {
  /** 模型列表内显示的简短标签，例如「API Key 无效或未授权 (401)」 */
  label: string;
  /** 悬浮提示的完整信息，包含分类说明与原始调试信息 */
  title: string;
}

/**
 * 将模型测速失败结果归类为友好中文信息。
 * - label：列表内短标签，限制 2-4 字（空间有限）
 * - title：悬浮提示的完整信息，含分类说明与原始调试信息
 */
export function describeTestFailure(statusCode?: number, reason?: string): TestFailureInfo {
  const raw = reason?.trim() || '未知错误';

  if (statusCode == null) {
    const short = shortNonHttpFailure(raw);
    const full = classifyNonHttpFailure(raw);
    return { label: short, title: `${full}\n\n调试信息：\n${raw}` };
  }

  const short = shortHttpStatus(statusCode);
  const full = describeHttpStatus(statusCode);
  return { label: short, title: `${full}\n\n调试信息：\n${raw}` };
}

/** 将 HTTP 状态码归类为完整中文描述（用于悬浮提示） */
function describeHttpStatus(code: number): string {
  if (code >= 500) {
    switch (code) {
      case 502:
        return '网关错误 (502)';
      case 503:
        return '服务暂不可用 (503)';
      case 504:
        return '网关超时 (504)';
      default:
        return `上游服务器错误 (${code})`;
    }
  }
  if (code >= 400) {
    switch (code) {
      case 400:
        return '请求格式错误 (400)';
      case 401:
        return 'API Key 无效或未授权 (401)';
      case 403:
        return '无访问权限 (403)';
      case 404:
        return '接口不存在，检查 Base URL (404)';
      case 408:
        return '请求超时 (408)';
      case 429:
        return '请求过于频繁，已被限流 (429)';
      default:
        return `客户端错误 (${code})`;
    }
  }
  return `HTTP ${code}`;
}

/** 将 HTTP 状态码归类为 2-4 字短标签（用于列表内显示） */
function shortHttpStatus(code: number): string {
  if (code >= 500) {
    switch (code) {
      case 502:
        return '网关错';
      case 503:
        return '暂不可用';
      case 504:
        return '网关超时';
      default:
        return '服务端错';
    }
  }
  if (code >= 400) {
    switch (code) {
      case 400:
        return '格式错';
      case 401:
        return '未授权';
      case 403:
        return '无权限';
      case 404:
        return '地址错';
      case 408:
        return '超时';
      case 429:
        return '限流';
      default:
        return '客户端错';
    }
  }
  return `HTTP ${code}`;
}

/** 无 HTTP 状态码（请求未到达服务端）时按原因文本归类为完整描述 */
function classifyNonHttpFailure(raw: string): string {
  const lower = raw.toLowerCase();
  if (raw.includes('Header')) return 'Header 配置错误';
  if (lower.includes('timeout')) return '连接超时';
  if (lower.includes('request failed') || lower.includes('network') || lower.includes('unreachable')) {
    return '网络不可达';
  }
  return '连接失败';
}

/** 无 HTTP 状态码时按原因文本归类为 2-4 字短标签 */
function shortNonHttpFailure(raw: string): string {
  const lower = raw.toLowerCase();
  if (raw.includes('Header')) return '配置错';
  if (lower.includes('timeout')) return '超时';
  if (lower.includes('request failed') || lower.includes('network') || lower.includes('unreachable')) {
    return '网络错';
  }
  return '连接错';
}
