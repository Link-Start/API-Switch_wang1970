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
