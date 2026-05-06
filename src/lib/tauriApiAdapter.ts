import type { ApiAdapter } from './apiAdapter';
import type { Channel, CreateChannelParams, UpdateChannelParams, FetchModelsResult, ProbeResult, ModelInfo, ModelCatalogMetaUpdate } from '../features/channels/types';
import type { DashboardFilter, DashboardStats, ChartDataPoint, ModelRanking, UsageLog, UsageLogFilter, PaginatedResult, ApiEntry, AccessKey, TranslationRelayPayload, TranslationRelayRequest } from '../types';
import { ADMIN_API_PREFIX } from './adminApiConfig';
import {
    listChannels,
    createChannel,
    updateChannel,
    deleteChannel,
    fetchModels,
    fetchModelsDirect,
    selectModels,
    probeUrl,
    updateChannelResponseMs,
    listEntries,
    toggleEntry,
    reorderEntries,
    createEntry,
    deleteEntry,
    testEntryLatency,
    backfillEntryCatalogMeta,
    listAccessKeys,
    createAccessKey,
    deleteAccessKey,
    toggleAccessKey,
    getUsageLogs,
    getDashboardStats,
    getModelConsumption,
    getCallTrend,
    getModelDistribution,
    getUserTrend,
    getSettings,
    updateSettings,
    getProxyStatus,
    startProxy,
    stopProxy,
    testChat,
    getGroups,
    updateGroup,
    translateAndRelay,
    getLatestTranslation,
} from './api';

export const tauriApiAdapter: ApiAdapter = {
    channels: {
        async list() {
            return await listChannels();
        },
        async create(params) {
            return await createChannel(params);
        },
        async update(params) {
            return await updateChannel(params);
        },
        async delete(id) {
            await deleteChannel(id);
        },
        async fetchModels(channelId) {
            return await fetchModels(channelId);
        },
        async fetchModelsDirect(apiType, baseUrl, apiKey, verified) {
            return await fetchModelsDirect(apiType, baseUrl, apiKey, verified);
        },
        async probeUrl(url) {
            return await probeUrl(url);
        },
        async selectModels(channelId, modelNames, availableModels, catalogMeta = []) {
            await selectModels(channelId, modelNames, availableModels, catalogMeta);
        },
        async updateResponseMs(channelId, responseMs) {
            await updateChannelResponseMs(channelId, responseMs);
        },
    },
    usage: {
        getLogs: getUsageLogs,
        getDashboardStats: getDashboardStats,
        getModelConsumption: getModelConsumption,
        getCallTrend: getCallTrend,
        getModelDistribution: getModelDistribution,
        getUserTrend: getUserTrend,
    },
    pool: {
        list: listEntries,
        toggle: (id, enabled) => toggleEntry(id, enabled),
        reorder: reorderEntries,
        create: (params) => createEntry({ channel_id: params.channelId, model: params.model, display_name: params.displayName, group_name: params.groupName }),
        delete: deleteEntry,
        testLatency: async (id) => {
            const result = await testEntryLatency(id);
            return {
                entry_id: id,
                latency_ms: result.status === 'ok' && result.response_ms !== 'X' ? parseInt(result.response_ms, 10) : null,
            };
        },
        backfillCatalogMeta: (items) =>
            backfillEntryCatalogMeta(
                items.map((item) => ({
                    id: item.entryId,
                    provider_logo: '',
                    release_date: '',
                    model_meta_zh: '',
                    model_meta_en: '',
                }))
            ),
        getGroups: getGroups,
        updateGroup: updateGroup,
    },
  tokens: {
    list: listAccessKeys,
    create: createAccessKey,
    delete: deleteAccessKey,
    toggle: toggleAccessKey,
  },
settings: {
    get: getSettings,
    update: updateSettings,
    patchSettings: async (patch) => {
        // Tauri command 使用完整对象，fallback 到完整 update
        const current = await getSettings();
        await updateSettings({ ...current, ...patch });
        return { ...current, ...patch };
    },
},
  proxy: {
    getStatus: getProxyStatus,
    start: startProxy,
    stop: stopProxy,
  },
  testChat: (entryId, messages) => testChat(entryId, messages),
    translation: {
        getLatest: getLatestTranslation,
        translateAndRelay: (request: TranslationRelayRequest) => translateAndRelay(request),
    },
    // Uses raw fetch instead of Tauri invoke because there is no `get_version` command.
    // In Combined mode the admin server is merged into the proxy port, so /admin/version works.
    // Path constructed from ADMIN_API_PREFIX to match the Rust route in admin/router.rs.
    async getVersion() {
      const response = await fetch(`${ADMIN_API_PREFIX}/version`);
      return response.json();
    },
};
