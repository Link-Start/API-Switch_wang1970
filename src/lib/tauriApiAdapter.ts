import { invoke } from '@tauri-apps/api/core';
import type { ApiAdapter } from './apiAdapter';
import type { Channel, CreateChannelParams, UpdateChannelParams, FetchModelsResult, ProbeResult, ModelInfo, ModelCatalogMetaUpdate } from '../features/channels/types';

export const tauriApiAdapter: ApiAdapter = {
  channels: {
    async list() {
      return await invoke<Channel[]>('list_channels');
    },
    async create(params) {
      return await invoke<Channel>('create_channel', { params });
    },
    async update(params) {
      return await invoke<Channel>('update_channel', { params });
    },
    async delete(id) {
      await invoke('delete_channel', { id });
    },
    async fetchModels(channelId) {
      return await invoke<FetchModelsResult>('fetch_models', { channelId });
    },
    async fetchModelsDirect(apiType, baseUrl, apiKey, verified) {
      return await invoke<FetchModelsResult>('fetch_models_direct', { apiType, baseUrl, apiKey, verified });
    },
    async probeUrl(url) {
      return await invoke<ProbeResult>('probe_url', { url });
    },
    async selectModels(channelId, modelNames, availableModels, catalogMeta = []) {
      await invoke('select_models', { channelId, modelNames, availableModels, catalogMeta });
    },
    async updateResponseMs(channelId, responseMs) {
      await invoke('update_channel_response_ms', { channelId, responseMs });
    },
  },
};
