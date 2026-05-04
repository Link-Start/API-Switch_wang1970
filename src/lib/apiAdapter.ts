export interface ApiAdapter {
  channels: {
    list(): Promise<Channel[]>;
    create(params: CreateChannelParams): Promise<Channel>;
    update(params: UpdateChannelParams): Promise<Channel>;
    delete(id: string): Promise<void>;
    fetchModels(channelId: string): Promise<FetchModelsResult>;
    fetchModelsDirect(apiType: string, baseUrl: string, apiKey: string, verified?: boolean): Promise<FetchModelsResult>;
    probeUrl(url: string): Promise<ProbeResult>;
    selectModels(channelId: string, modelNames: string[], availableModels: ModelInfo[], catalogMeta?: ModelCatalogMetaUpdate[]): Promise<void>;
    updateResponseMs(channelId: string, responseMs: string): Promise<void>;
  };
  usage: {
    getLogs(filter: UsageLogFilter): Promise<PaginatedResult<UsageLog>>;
    getDashboardStats(filter?: DashboardFilter): Promise<DashboardStats>;
    getModelConsumption(filter?: DashboardFilter): Promise<ChartDataPoint[]>;
    getCallTrend(filter?: DashboardFilter): Promise<ChartDataPoint[]>;
    getModelDistribution(filter?: DashboardFilter): Promise<ModelRanking[]>;
    getUserTrend(filter?: DashboardFilter): Promise<ChartDataPoint[]>;
  };
}

// Types referenced above – import from shared definitions
import type { Channel, CreateChannelParams, UpdateChannelParams, FetchModelsResult, ProbeResult, ModelInfo, ModelCatalogMetaUpdate } from '../features/channels/types';
import type { DashboardFilter, DashboardStats, ChartDataPoint, ModelRanking, UsageLog, UsageLogFilter, PaginatedResult } from '../types';
