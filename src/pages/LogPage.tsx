import { useState, Fragment } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Card, CardContent } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { toast } from 'sonner';
import { useApiAdapter } from "@/lib/useApiAdapter";
import { useDirtyPolling } from "../lib/useDirtyPolling";
import type { UsageLogFilter } from "@/types";

export function LogPage() {
  const { t } = useTranslation();
  const api = useApiAdapter();
  const [filter, setFilter] = useState<UsageLogFilter>({ page: 1, page_size: 100 });
  const [errorsOnly, setErrorsOnly] = useState(false);
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [showClearDialog, setShowClearDialog] = useState(false);
  const [isClearing, setIsClearing] = useState(false);

  useDirtyPolling('log');

  const { data: result, isLoading } = useQuery({
    queryKey: ["usageLogs", filter],
    queryFn: () => api.usage.getLogs(filter),
  });

  const todayStart = new Date();
  todayStart.setHours(0, 0, 0, 0);
  const todayEnd = new Date(todayStart);
  todayEnd.setDate(todayEnd.getDate() + 1);
  const todayFilter = {
    start_time: Math.floor(todayStart.getTime() / 1000),
    end_time: Math.floor(todayEnd.getTime() / 1000) - 1,
  };
  const { data: todayStats } = useQuery({
    queryKey: ["usageLogs", "todayStats", todayFilter.start_time, todayFilter.end_time],
    queryFn: () => api.usage.getDashboardStats(todayFilter),
  });

  const logs = result?.items || [];

  if (isLoading) {
    return (
      <div className="p-6">
        <div className="flex items-center justify-between mb-6">
          <div className="h-6 w-32 animate-pulse bg-muted rounded" />
          <div className="flex items-center gap-2">
            <div className="h-4 w-12 animate-pulse bg-muted rounded" />
            <div className="h-4 w-12 animate-pulse bg-muted rounded" />
          </div>
        </div>

        <div className="grid grid-cols-2 xl:grid-cols-4 gap-4 mb-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <Card key={i}>
              <CardContent className="p-4">
                <div className="h-4 w-24 animate-pulse bg-muted rounded mb-2" />
                <div className="h-8 w-16 animate-pulse bg-muted rounded" />
              </CardContent>
            </Card>
          ))}
        </div>

        <div className="rounded-md border overflow-x-hidden">
          <table className="w-full table-fixed text-sm">
            <colgroup>
              <col className="w-40" />
              <col className="w-28" />
              <col className="w-24" />
              <col />
              <col className="w-28" />
              <col className="w-16" />
              <col className="w-16" />
            <col className="w-10" />
            </colgroup>
            <thead>
              <tr className="border-b bg-muted/50">
                <th className="px-3 py-2 text-left font-medium whitespace-nowrap"><div className="h-4 w-20 animate-pulse bg-muted rounded" /></th>
                <th className="px-3 py-2 text-left font-medium truncate"><div className="h-4 w-16 animate-pulse bg-muted rounded" /></th>
                <th className="px-3 py-2 text-left font-medium truncate"><div className="h-4 w-12 animate-pulse bg-muted rounded" /></th>
                <th className="px-3 py-2 text-left font-medium truncate"><div className="h-4 w-24 animate-pulse bg-muted rounded" /></th>
                <th className="px-3 py-2 text-left font-medium whitespace-nowrap"><div className="h-4 w-16 animate-pulse bg-muted rounded" /></th>
                <th className="px-3 py-2 text-right font-medium"><div className="h-4 w-12 animate-pulse bg-muted rounded ml-auto" /></th>
                <th className="px-3 py-2 text-right font-medium"><div className="h-4 w-12 animate-pulse bg-muted rounded ml-auto" /></th>
                <th className="px-3 py-2 text-left font-medium whitespace-nowrap"><div className="h-4 w-14 animate-pulse bg-muted rounded" /></th>
              </tr>
            </thead>
            <tbody>
              {Array.from({ length: 5 }).map((_, i) => (
                <tr key={i} className="border-b">
                  <td className="px-3 py-2 whitespace-nowrap"><div className="h-4 w-32 animate-pulse bg-muted rounded" /></td>
                  <td className="px-3 py-2 min-w-0"><div className="h-4 w-20 animate-pulse bg-muted rounded" /></td>
                  <td className="px-3 py-2 min-w-0"><div className="h-4 w-16 animate-pulse bg-muted rounded" /></td>
                  <td className="px-3 py-2 font-mono text-xs min-w-0"><div className="h-4 w-24 animate-pulse bg-muted rounded" /></td>
                  <td className="px-3 py-2 whitespace-nowrap"><div className="h-4 w-28 animate-pulse bg-muted rounded" /></td>
                  <td className="px-3 py-2 text-right"><div className="h-4 w-10 animate-pulse bg-muted rounded ml-auto" /></td>
                  <td className="px-3 py-2 text-right"><div className="h-4 w-10 animate-pulse bg-muted rounded ml-auto" /></td>
                  <td className="px-3 py-2 whitespace-nowrap"><div className="h-4 w-12 animate-pulse bg-muted rounded" /></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    );
  }

  const toggleErrorsOnly = (checked: boolean) => {
    setErrorsOnly(checked);
    setFilter((f) => ({
      ...f,
      success: checked ? false : undefined,
      page: 1,
    }));
  };

  const handleClearDetails = async () => {
    setIsClearing(true);
    try {
      const count = await api.usage.clearLogDetails();
      toast.success(t("log.clearDone", { count }));
      setShowClearDialog(false);
    } catch (e) {
      console.error("clearLogDetails failed:", e);
      toast.error(t("log.clearFailed"));
    } finally {
      setIsClearing(false);
    }
  };

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold">{t("log.title")}</h1>
        <Button variant="outline" onClick={() => setShowClearDialog(true)}>{t("log.clearData")}</Button>
      </div>

      <div className="grid gap-4 md:grid-cols-4 mb-4">
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.recentLogs")}</div>
            <div className="text-2xl font-semibold mt-1">{todayStats?.total_requests ?? 0}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.promptTokens")}</div>
            <div className="text-2xl font-semibold mt-1">{todayStats?.total_prompt_tokens ?? 0}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.completionTokens")}</div>
            <div className="text-2xl font-semibold mt-1">{todayStats?.total_completion_tokens ?? 0}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.successRate")}</div>
            <div className="text-2xl font-semibold mt-1">{todayStats && todayStats.total_requests > 0 ? `${(todayStats.success_rate * 100).toFixed(1)}%` : "0%"}</div>
          </CardContent>
        </Card>
      </div>

      <div className="rounded-md border overflow-x-hidden">
        <table className="w-full table-fixed text-sm">
          <colgroup>
            <col className="w-40" />
            <col className="w-28" />
            <col className="w-24" />
            <col />
            <col className="w-28" />
            <col className="w-16" />
            <col className="w-16" />
            <col className="w-20" />
          </colgroup>
          <thead>
            <tr className="border-b bg-muted/50">
              <th className="px-3 py-2 text-left font-medium whitespace-nowrap">{t("log.time")}</th>
              <th className="px-3 py-2 text-left font-medium truncate">{t("log.channel")}</th>
              <th className="px-3 py-2 text-left font-medium truncate">{t("log.token")}</th>
              <th className="px-3 py-2 text-left font-medium truncate">{t("log.model")}</th>
              <th className="px-3 py-2 text-left font-medium whitespace-nowrap">{t("log.duration")}</th>
              <th className="px-3 py-2 text-right font-medium">{t("log.promptTokens")}</th>
              <th className="px-3 py-2 text-right font-medium">{t("log.completionTokens")}</th>
              <th className="px-1 py-1 text-left font-medium whitespace-nowrap"><Switch checked={errorsOnly} onCheckedChange={toggleErrorsOnly} /></th>
            </tr>
          </thead>
          <tbody>
            {logs.map((log) => {
              const isExpanded = expandedId === log.id;
              const resolvedModel = log.model;
              return (
                <Fragment key={log.id}>
                  <tr className="border-b hover:bg-muted/30 cursor-pointer" onClick={() => setExpandedId(isExpanded ? null : log.id)}>
                    <td className="px-3 py-2 whitespace-nowrap"><div>{new Date(log.created_at * 1000).toLocaleString()}</div></td>
                    <td className="px-3 py-2 min-w-0"><div className="truncate" title={log.channel_name}>{log.channel_name}</div></td>
                    <td className="px-3 py-2 min-w-0"><div className="truncate" title={log.token_name || log.access_key_name || undefined}>{log.token_name || log.access_key_name || <span className="text-muted-foreground">-</span>}</div></td>
                    <td className="px-3 py-2 font-mono text-xs min-w-0"><div className="truncate" title={resolvedModel}>{resolvedModel}</div></td>
                    <td className="px-3 py-2 whitespace-nowrap"><div>{`${log.use_time || Math.ceil(log.latency_ms / 1000)}s${log.is_stream && log.first_token_ms > 0 ? ` / ${(log.first_token_ms / 1000).toFixed(1)}s` : ""}  ${log.is_stream ? t("log.streamShort") : t("log.nonStreamShort")}`}</div></td>
                    <td className="px-3 py-2 text-right">{log.prompt_tokens}</td>
                    <td className="px-3 py-2 text-right">{log.completion_tokens}</td>
                    <td className="px-3 py-2 whitespace-nowrap"><span className={log.success ? "text-green-600" : "text-red-500"}>{log.success ? t("log.success") : t("log.failed")}</span></td>
                  </tr>
                  {isExpanded ? (
                    <tr className="border-b bg-muted/20">
                      <td colSpan={8} className="px-4 py-3">
                        <div className="space-y-2 text-xs max-w-3xl">
                          {log.content ? (
                            <div>
                              <div className="font-medium text-muted-foreground mb-1">{t("log.details")}</div>
                              <pre className="whitespace-pre-wrap break-all">{log.content}</pre>
                            </div>
                          ) : null}
                          {log.error_message ? (
                            <div>
                              <div className="font-medium text-red-500 mb-1">{t("log.error")}</div>
                              <pre className="whitespace-pre-wrap break-all text-red-500">{log.error_message}</pre>
                            </div>
                          ) : null}
                          {!log.content && !log.error_message ? <span className="text-muted-foreground">{t("log.noError")}</span> : null}
                        </div>
                      </td>
                    </tr>
                  ) : null}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      </div>

      {!logs.length && !isLoading && (
        <div className="flex h-32 items-center justify-center text-muted-foreground">{t("common.noData")}</div>
      )}

      <Dialog open={showClearDialog} onOpenChange={setShowClearDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("log.clearConfirmTitle")}</DialogTitle>
          </DialogHeader>
          <p className="text-sm text-muted-foreground">{t("log.clearConfirm")}</p>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowClearDialog(false)}>{t("common.cancel")}</Button>
            <Button variant="destructive" disabled={isClearing} onClick={handleClearDetails}>{t("log.clearData")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}


