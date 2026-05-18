/**
 * 渠道编辑器模块
 * 独立抽离的渠道创建和编辑功能
 */

export { ChannelEditorDialog } from './ChannelEditorDialog';
export { channelToForm } from './types';
export type { ChannelFormState, UrlProbeResult } from './types';
export { DEFAULT_FORM, API_TYPES } from './types';
export { formatReleaseDate, buildEntryCatalogMeta } from './utils';
