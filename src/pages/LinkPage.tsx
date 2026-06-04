import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Check, Copy, ExternalLink, Link, Loader2, MessageSquare, Terminal } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useApiAdapter } from "@/lib/useApiAdapter";
import type { AppConfigResult, ConnectionAppItem } from "@/types";

const ICONS = {
  ExternalLink,
  Terminal,
  MessageSquare,
  Link,
} as const;

function AppIcon({ icon }: { icon: string }) {
  const Icon = ICONS[icon as keyof typeof ICONS] ?? Link;
  return <Icon className="h-5 w-5 text-primary" />;
}

export function LinkPage() {
  const { t } = useTranslation();
  const api = useApiAdapter();
  const [confirmApp, setConfirmApp] = useState<ConnectionAppItem | null>(null);
  const [clipboardResult, setClipboardResult] = useState<{ app: ConnectionAppItem; result: AppConfigResult } | null>(null);
  const [copied, setCopied] = useState(false);

  const { data: apps = [], isLoading } = useQuery({
    queryKey: ["connectionApps"],
    queryFn: () => api.connectionApps.list(),
  });

  const visibleApps = apps;

  const executeMutation = useMutation({
    mutationFn: (app: ConnectionAppItem) => api.connectionApps.execute(app.id).then((result) => ({ app, result })),
    onSuccess: ({ app, result }) => {
      setConfirmApp(null);
      if (result.action === "write") {
        if (result.file_path) toast.success(t("link.success", { filePath: result.file_path }));
        if (result.backup_path) toast.message(t("link.backup", { backupPath: result.backup_path }));
        return;
      }
      setCopied(false);
      setClipboardResult({ app, result });
    },
    onError: (err) => {
      const message = err instanceof Error ? err.message : String(err);
      toast.error(t("link.error.write_failed", { reason: message }));
    },
  });

  const startConnect = (app: ConnectionAppItem) => {
    if (app.status === "coming_soon") return;
    if (app.configMode === "write") {
      setConfirmApp(app);
      return;
    }
    executeMutation.mutate(app);
  };

  const copyContent = async () => {
    const content = clipboardResult?.result.content;
    if (!content) return;
    await navigator.clipboard.writeText(content);
    setCopied(true);
    toast.success(t("link.clipboard.copied"));
  };

  if (isLoading) {
    return <div className="p-6 text-muted-foreground">{t("common.loading")}</div>;
  }

  return (
    <div className="p-4 sm:p-6">
      <div className="mb-4 sm:mb-6">
        <h1 className="text-xl font-semibold">{t("link.title")}</h1>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
        {visibleApps.map((app) => {
          const pending = executeMutation.isPending && executeMutation.variables?.id === app.id;
          const disabled = executeMutation.isPending || app.status === "coming_soon";
          return (
            <Card key={app.id} className="flex min-h-36 flex-col">
              <CardHeader className="pb-3">
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="flex min-w-0 items-center gap-3">
                    <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10">
                      <AppIcon icon={app.icon} />
                    </div>
                    <CardTitle className="truncate text-base">{app.name}</CardTitle>
                  </div>
                  <Button size="sm" className="shrink-0" disabled={disabled} onClick={() => startConnect(app)} title={app.status === "coming_soon" ? t("link.comingSoon") : undefined}>
                    {pending ? <Loader2 className="mr-1.5 h-4 w-4 animate-spin" /> : null}
                    {t("link.connect")}
                  </Button>
                </div>
              </CardHeader>
              <CardContent className="pt-0">
                <CardDescription className="line-clamp-2">{app.description}</CardDescription>
              </CardContent>
            </Card>
          );
        })}
      </div>

      <Dialog open={!!confirmApp} onOpenChange={(open) => !open && setConfirmApp(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("link.confirm.title", { appName: confirmApp?.name ?? "" })}</DialogTitle>
            <DialogDescription>
              {t("link.confirm.description", { filePath: confirmApp?.config.file ?? "" })}
            </DialogDescription>
          </DialogHeader>
          <p className="text-sm text-muted-foreground">{t("link.confirm.autoKey")}</p>
          <DialogFooter>
            <Button variant="outline" disabled={executeMutation.isPending} onClick={() => setConfirmApp(null)}>
              {t("common.cancel")}
            </Button>
            <Button disabled={executeMutation.isPending || !confirmApp} onClick={() => confirmApp && executeMutation.mutate(confirmApp)}>
              {executeMutation.isPending ? <Loader2 className="mr-1.5 h-4 w-4 animate-spin" /> : null}
              {t("common.confirm")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={!!clipboardResult} onOpenChange={(open) => !open && setClipboardResult(null)}>
        <DialogContent className="flex max-h-[calc(100dvh-1rem)] flex-col overflow-hidden sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>{t("link.clipboard.title", { appName: clipboardResult?.app.name ?? "" })}</DialogTitle>
            <DialogDescription>{clipboardResult?.result.instructions}</DialogDescription>
          </DialogHeader>
          <ScrollArea className="min-h-0 flex-1 rounded-md border bg-muted/30">
            <pre className="p-4 text-xs whitespace-pre-wrap break-words"><code>{clipboardResult?.result.content}</code></pre>
          </ScrollArea>
          <DialogFooter>
            <Button variant="outline" onClick={() => setClipboardResult(null)}>{t("common.close")}</Button>
            <Button onClick={copyContent}>
              {copied ? <Check className="mr-1.5 h-4 w-4" /> : <Copy className="mr-1.5 h-4 w-4" />}
              {copied ? t("link.clipboard.copied") : t("link.clipboard.copy")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
