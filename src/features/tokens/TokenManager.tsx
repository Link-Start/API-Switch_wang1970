import { useState, useMemo } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, Copy, Check, MoreVertical } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from "@/components/ui/dialog";
import { useApiAdapter } from "@/lib/useApiAdapter";
import { useDirtyPolling } from "@/lib/useDirtyPolling";
import { toast } from "sonner";
import type { AccessKey } from "@/types";

export function TokenManager() {
  const { t } = useTranslation();
  const adapter = useApiAdapter();
  const queryClient = useQueryClient();

  const [showCreate, setShowCreate] = useState(false);
  const [newKeyName, setNewKeyName] = useState("");
  const [createdKey, setCreatedKey] = useState<AccessKey | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const dirtyQueryKeys = useMemo(() => [["accessKeys"]] as const, []);
  useDirtyPolling('token', dirtyQueryKeys);

  const { data: keys = [], isLoading } = useQuery({
    queryKey: ["accessKeys"],
    queryFn: () => adapter.tokens.list(),
    staleTime: 2000,
  });

  const createMutation = useMutation({
    mutationFn: (name: string) => adapter.tokens.create(name),
    onSuccess: (key) => {
      queryClient.invalidateQueries({ queryKey: ["accessKeys"] });
      setShowCreate(false);
      setCreatedKey(key);
      setNewKeyName("");
    },
    onError: (err) => {
      toast.error(`${t("token.add")} ${t("common.failed")}: ${err}`);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => adapter.tokens.delete(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["accessKeys"] });
    },
    onError: (err) => {
      toast.error(`${t("common.delete")} ${t("common.failed")}: ${err}`);
    },
  });

  const toggleMutation = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      adapter.tokens.toggle(id, enabled),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["accessKeys"] }),
    onError: (err) => {
      toast.error(`${t("common.toggle")} ${t("common.failed")}: ${err}`);
    },
  });

  const copyKey = async (key: string, id: string) => {
    await navigator.clipboard.writeText(key);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 3000);
  };

  const formatDate = (ts: number) => new Date(ts * 1000).toLocaleString();

  const shortKey = (key: string) => {
    if (key.length <= 14) return key;
    return `${key.slice(0, 6)}…${key.slice(-4)}`;
  };

  if (isLoading) {
    return <div className="p-6 text-muted-foreground">{t("common.loading")}</div>;
  }

  return (
    <div className="p-4 sm:p-6">
      <div className="mb-6 flex items-center justify-between gap-3">
        <h1 className="text-xl font-semibold">{t("token.title")}</h1>
        <Button size="sm" className="gap-1.5" onClick={() => setShowCreate(true)}>
          <Plus className="h-4 w-4" />
          {t("token.add")}
        </Button>
      </div>

      {keys.length ? (
        <div className="hidden overflow-hidden rounded-lg border md:block">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b bg-muted/50 text-left text-muted-foreground">
                <th className="px-4 py-2 font-medium w-16">{t("token.enabled")}</th>
                <th className="px-4 py-2 font-medium">{t("token.name")}</th>
                <th className="px-4 py-2 font-medium">{t("token.key")}</th>
                <th className="px-4 py-2 font-medium">{t("token.created")}</th>
                <th className="px-4 py-2 font-medium w-16">{t("common.action")}</th>
              </tr>
            </thead>
            <tbody>
              {keys.map((k) => (
                <tr key={k.id} className="border-b last:border-b-0 hover:bg-muted/30">
                  <td className="px-4 py-3">
                    <Switch
                      checked={k.enabled}
                      onCheckedChange={(checked) => toggleMutation.mutate({ id: k.id, enabled: checked })}
                    />
                  </td>
                  <td className="px-4 py-3 font-medium">{k.name}</td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-1 min-w-0">
                      <code className="text-xs bg-muted px-1.5 py-0.5 rounded font-mono break-all flex-1 min-w-0">
                        {k.key}
                      </code>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-5 w-5 shrink-0 text-muted-foreground"
                        onClick={() => copyKey(k.key, k.id)}
                      >
                        {copiedId === k.id ? (
                          <Check className="h-3 w-3 text-green-600" />
                        ) : (
                          <Copy className="h-3 w-3" />
                        )}
                      </Button>
                    </div>
                  </td>
                  <td className="px-4 py-3 text-muted-foreground text-xs">
                    {formatDate(k.created_at)}
                  </td>
                  <td className="px-4 py-3">
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7"
                      onClick={() => deleteMutation.mutate(k.id)}
                    >
                      <Trash2 className="h-3.5 w-3.5 text-destructive" />
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}

      {keys.length ? (
        <div className="space-y-2 md:hidden">
          {keys.map((k) => (
            <div key={k.id} className="rounded-lg border bg-background p-3">
              <div className="flex items-center gap-3">
                <Switch
                  checked={k.enabled}
                  onCheckedChange={(checked) => toggleMutation.mutate({ id: k.id, enabled: checked })}
                />
                <div className="min-w-0 flex-1 truncate font-medium">{k.name}</div>
                <code className="max-w-[34vw] shrink-0 truncate rounded bg-muted px-1.5 py-0.5 font-mono text-xs text-muted-foreground">
                  {shortKey(k.key)}
                </code>
                <Popover>
                  <PopoverTrigger asChild>
                    <Button variant="ghost" size="icon" className="h-9 w-9 shrink-0">
                      <MoreVertical className="h-4 w-4" />
                    </Button>
                  </PopoverTrigger>
                  <PopoverContent align="end" className="w-40 p-1">
                    <button type="button" className="flex w-full items-center gap-2 rounded px-2 py-2 text-left text-sm hover:bg-accent" onClick={() => copyKey(k.key, k.id)}>
                      {copiedId === k.id ? <Check className="h-4 w-4 text-green-600" /> : <Copy className="h-4 w-4" />}
                      {t("common.copy")}
                    </button>
                    <button type="button" className="flex w-full items-center gap-2 rounded px-2 py-2 text-left text-sm text-destructive hover:bg-accent" onClick={() => deleteMutation.mutate(k.id)}>
                      <Trash2 className="h-4 w-4" />
                      {t("common.delete")}
                    </button>
                  </PopoverContent>
                </Popover>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="flex h-64 items-center justify-center text-muted-foreground">
          {t("common.noData")}
        </div>
      )}

      {/* Create Dialog */}
      <Dialog open={showCreate} onOpenChange={setShowCreate}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("token.add")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <Label>{t("token.name")}</Label>
              <Input
                value={newKeyName}
                onChange={(e) => setNewKeyName(e.target.value)}
                placeholder={t("token.deviceNamePlaceholder")}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowCreate(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              onClick={() => createMutation.mutate(newKeyName)}
              disabled={!newKeyName.trim() || createMutation.isPending}
            >
              {t("common.add")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Created Key Dialog */}
      <Dialog open={!!createdKey} onOpenChange={(v) => !v && setCreatedKey(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("token.add")}</DialogTitle>
            <DialogDescription>{t("token.keyWarning")}</DialogDescription>
          </DialogHeader>
          {createdKey && (
            <div className="space-y-3">
              <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
                <code className="flex-1 text-sm bg-muted p-3 rounded font-mono break-all">
                  {createdKey.key}
                </code>
                <Button
                  variant="outline"
                  size="icon"
                  onClick={() => copyKey(createdKey.key, createdKey.id)}
                >
                  {copiedId === createdKey.id ? (
                    <Check className="h-4 w-4 text-green-600" />
                  ) : (
                    <Copy className="h-4 w-4" />
                  )}
                </Button>
              </div>
            </div>
          )}
          <DialogFooter>
            <Button onClick={() => setCreatedKey(null)}>{t("common.close")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
