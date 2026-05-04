import { invoke } from '@tauri-apps/api/core';
import type { ApiAdapter } from './apiAdapter';
import type { Channel, CreateChannelParams, UpdateChannelParams, FetchModelsResult, ProbeResult, ModelInfo, ModelCatalogMetaUpdate } from '../features/channels/types';
import type { DashboardFilter, DashboardStats, ChartDataPoint, ModelRanking, UsageLog, UsageLogFilter, PaginatedResult } from '../types';

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
  usage: {
    async getLogs(filter) {
      return await invoke<PaginatedResult<UsageLog>>('get_usage_logs', { filter });
    },
    async getDashboardStats(filter) {
      return await invoke<DashboardStats>('get_dashboard_stats', { filter });
    },
    async getModelConsumption(filter) {
      return await invoke<ChartDataPoint[]>('get_model_consumption', { filter });
    },
    async getCallTrend(filter) {
      return await invoke<ChartDataPoint[]>('get_call_trend', { filter });
    },
    async getModelDistribution(filter) {
      return await invoke<ModelRanking[]>('get_model_distribution', { filter });
    },
    async getUserTrend(filter) {
      return await invoke<ChartDataPoint[]>('get_user_trend', { filter });
    },
  },
};
