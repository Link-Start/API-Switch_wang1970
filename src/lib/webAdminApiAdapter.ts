import type { ApiAdapter } from './apiAdapter';
import type {
  Channel,
  ChannelOperationError,
  ChannelOperationErrorKind,
  ChannelOperationHttpError,
  FetchModelsResult,
  ProbeResult,
  ModelInfo,
  ModelCatalogMetaUpdate,
} from '../features/channels/types';

const apiBase = '/admin';

interface ErrorEnvelope {
  error?: ChannelOperationError;
}

function classifyChannelOperationError(status: number, error?: ChannelOperationError): ChannelOperationErrorKind {
  switch (error?.code) {
    case 'INVALID_CREDENTIALS':
    case 'UNAUTHORIZED':
    case 'FORBIDDEN':
      return 'auth';
    case 'TIMEOUT':
      return 'timeout';
    case 'ENDPOINT_UNREACHABLE':
      return 'network';
    case 'RATE_LIMITED':
      return 'rate_limited';
    case 'INVALID_URL':
      return 'invalid_url';
    case 'UNSUPPORTED_PROVIDER':
      return 'unsupported_provider';
    case 'EMPTY_MODEL_LIST':
      return 'empty_model_list';
    case 'ENDPOINT_CORRECTION_FAILED':
    case 'ENDPOINT_VALIDATION_FAILED':
      return 'endpoint_correction_failed';
    default:
      if (status === 401 || status === 403) {
        return 'auth';
      }
      return 'unknown';
  }
}

function createHttpError(status: number, fallbackMessage: string, error?: ChannelOperationError): ChannelOperationHttpError {
  const kind = classifyChannelOperationError(status, error);
  const instance = new Error(error?.message || fallbackMessage) as ChannelOperationHttpError;
  instance.name = 'ChannelOperationHttpError';
  instance.status = status;
  instance.error = error;
  instance.kind = kind;
  instance.isNetworkError = kind === 'network';
  instance.isAuthError = kind === 'auth';
  instance.isTimeoutError = kind === 'timeout';
  return instance;
}

async function request<T>(method: 'GET' | 'POST' | 'PUT' | 'DELETE', path: string, data?: unknown): Promise<T> {
  const token = localStorage.getItem('api-switch-web-admin-token');

  let response: Response;
  try {
    response = await fetch(`${apiBase}${path}`, {
      method,
      headers: {
        'Content-Type': 'application/json',
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
      body: data === undefined ? undefined : JSON.stringify(data),
    });
  } catch (cause) {
    const rawMessage = cause instanceof Error ? cause.message : String(cause);
    const error: ChannelOperationError = {
      code: 'ENDPOINT_UNREACHABLE',
      message: rawMessage,
      details: { path, method },
    };
    throw createHttpError(0, rawMessage, error);
  }

  if (!response.ok) {
    let error: ChannelOperationError | undefined;
    let message = `HTTP ${response.status}`;
    try {
      const body = (await response.json()) as ErrorEnvelope;
      error = body.error;
      message = error?.message || message;
    } catch {
      // ignore non-json response
    }
    throw createHttpError(response.status, message, error);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return response.json() as Promise<T>;
}

export const webAdminApiAdapter: ApiAdapter = {
  channels: {
    list: () => request<Channel[]>('GET', '/channels'),
    create: (params) => request<Channel>('POST', '/channels', params),
    update: (params) => request<Channel>('PUT', `/channels/${params.id}`, params),
    delete: (id) => request<void>('DELETE', `/channels/${id}`),
    fetchModels: (channelId) => request<FetchModelsResult>('POST', `/channels/${channelId}/fetch-models`),
    fetchModelsDirect: (apiType, baseUrl, apiKey, verified) =>
      request<FetchModelsResult>('POST', '/channels/fetch-models-direct', { apiType, baseUrl, apiKey, verified }),
    probeUrl: (url) => request<ProbeResult>('POST', '/channels/probe-url', { url }),
    selectModels: (channelId, modelNames, availableModels, catalogMeta = []) => request<void>('POST', `/channels/${channelId}/select-models`, {
      modelNames,
      availableModels,
      catalogMeta,
    }),
    updateResponseMs: (channelId, responseMs) => request<void>('PUT', `/channels/${channelId}/response-ms`, { channelId, responseMs }),
  },
};
