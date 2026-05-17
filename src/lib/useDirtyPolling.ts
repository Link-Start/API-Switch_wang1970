import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { apiAdapter } from './unifiedApiAdapter';

const DEFAULT_DIRTY_QUERY_KEYS: Record<'log' | 'pool' | 'channel' | 'token', readonly (readonly string[])[]> = {
  log: [['usageLogs']],
  pool: [['entries']],
  channel: [['channels']],
  token: [['accessKeys']],
};

/**
 * useDirtyPolling Hook
 * - 每 2 秒调用 `apiAdapter.dirty.take(module)` 检测脏标记。
 * - 当返回 true 时，使用 React Query 的 `queryClient.invalidateQueries` 刷新对应模块的查询。
 * - module 参数对应后端模块标识，可为 'log' | 'pool' | 'channel' | 'token'。
 * - queryKeys 可选：指定需要刷新的 query key 列表。
 */
export function useDirtyPolling(
  module: 'log' | 'pool' | 'channel' | 'token',
  queryKeys?: readonly (readonly string[])[],
) {
  const queryClient = useQueryClient();

  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      if (cancelled) return;
      try {
        const isDirty = await apiAdapter.dirty.take(module);
        if (isDirty) {
          const keys = queryKeys ?? DEFAULT_DIRTY_QUERY_KEYS[module];
          keys.forEach((key) => {
            queryClient.invalidateQueries({ queryKey: [...key] });
          });
        }
      } catch (e) {
        console.error('脏标记轮询失败:', e);
      } finally {
        if (!cancelled) {
          setTimeout(poll, 2000);
        }
      }
    };

    poll();
    return () => {
      cancelled = true;
    };
  }, [module, queryClient, queryKeys]);
}
