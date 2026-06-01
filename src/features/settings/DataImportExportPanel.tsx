import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, Copy } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useApiAdapter } from "@/lib/useApiAdapter";
import type { ChannelModelImportPreview } from "@/types";

export function DataImportExportPanel() {
  const adapter = useApiAdapter();
  const queryClient = useQueryClient();
  const [exportText, setExportText] = useState("");
  const [importText, setImportText] = useState("");
  const [preview, setPreview] = useState<ChannelModelImportPreview | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);

  const exportMutation = useMutation({
    mutationFn: () => adapter.importExport.exportChannelModel(),
    onSuccess: (payload) => {
      setExportText(payload);
      toast.success("导出完成，请复制下方 JSON 文本。");
    },
    onError: (error) => {
      toast.error(`导出失败：${error}`);
    },
  });

  const previewMutation = useMutation({
    mutationFn: (payload: string) => adapter.importExport.previewChannelModel(payload),
    onSuccess: (data) => {
      setPreview(data);
      setConfirmOpen(true);
    },
    onError: (error) => {
      toast.error(`导入校验失败：${error}`);
    },
  });

  const importMutation = useMutation({
    mutationFn: (payload: string) => adapter.importExport.importChannelModel(payload),
    onSuccess: (result) => {
      toast.success(result.message);
      setConfirmOpen(false);
      setPreview(null);
      setImportText("");
      queryClient.invalidateQueries({ queryKey: ["channels"] });
      queryClient.invalidateQueries({ queryKey: ["channels", "paginated"] });
      queryClient.invalidateQueries({ queryKey: ["channels", "all"] });
      queryClient.invalidateQueries({ queryKey: ["entries"] });
      queryClient.invalidateQueries({ queryKey: ["entries", "all"] });
      queryClient.invalidateQueries({ queryKey: ["pool-groups"] });
      queryClient.invalidateQueries({ queryKey: ["groups"] });
      queryClient.invalidateQueries({ queryKey: ["dashboardStats"] });
    },
    onError: (error) => {
      toast.error(`导入失败，请重新粘贴有效导出文本再次导入：${error}`);
    },
  });

  const isBusy = exportMutation.isPending || previewMutation.isPending || importMutation.isPending;

  const copyExportText = async () => {
    if (!exportText) return;
    try {
      await navigator.clipboard.writeText(exportText);
      toast.success("已复制到剪贴板。");
    } catch {
      toast.error("复制失败，请手动复制文本框内容。");
    }
  };

  const startImport = () => {
    const payload = importText.trim();
    if (!payload) {
      toast.error("请先粘贴 API Switch 导出的 JSON 文本。");
      return;
    }
    previewMutation.mutate(payload);
  };

  const confirmImport = () => {
    const payload = importText.trim();
    if (!payload) return;
    importMutation.mutate(payload);
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">数据导入导出</CardTitle>
      </CardHeader>
      <CardContent className="space-y-6">
        <div className="rounded-md border border-amber-200 bg-amber-50 p-3 text-sm text-amber-800">
          <div className="flex gap-2">
            <AlertTriangle className="mt-0.5 h-4 w-4 flex-shrink-0" />
            <div className="space-y-1">
              <p>仅迁移渠道和模型数据，不迁移系统设置、日志、token、连接应用或模型池。</p>
              <p>导出文本包含渠道 API Key，请自行妥善保管，不要发送给不可信的人。</p>
              <p>导入会清空当前设备上的所有渠道和模型，再使用粘贴文本重建。</p>
            </div>
          </div>
        </div>

        <section className="space-y-3">
          <div>
            <h3 className="font-medium">导出渠道和模型</h3>
            <p className="text-sm text-muted-foreground">
              点击后生成 JSON 文本，用户自行复制到导入设备。
            </p>
          </div>
          <div className="flex gap-2">
            <Button
              type="button"
              onClick={() => exportMutation.mutate()}
              disabled={isBusy}
            >
              {exportMutation.isPending ? "正在导出..." : "导出渠道和模型"}
            </Button>
            {exportText && (
              <Button type="button" variant="outline" onClick={copyExportText}>
                <Copy className="mr-2 h-4 w-4" />
                复制导出文本
              </Button>
            )}
          </div>
          {exportText && (
            <textarea
              value={exportText}
              readOnly
              rows={10}
              className="w-full rounded-md border bg-muted/40 p-3 font-mono text-xs"
            />
          )}
        </section>

        <section className="space-y-3 border-t pt-5">
          <div>
            <h3 className="font-medium">导入渠道和模型</h3>
            <p className="text-sm text-muted-foreground">
              只接受 API Switch 本功能导出的 JSON 文本。导入前会先完整校验，再请求确认覆盖。
            </p>
          </div>
          <textarea
            value={importText}
            onChange={(event) => setImportText(event.target.value)}
            rows={8}
            placeholder="粘贴 API Switch 导出的 JSON 文本..."
            className="w-full rounded-md border bg-background p-3 font-mono text-xs"
            disabled={isBusy}
          />
          <Button
            type="button"
            variant="destructive"
            onClick={startImport}
            disabled={!importText.trim() || isBusy}
          >
            {previewMutation.isPending ? "正在校验..." : "导入渠道和模型"}
          </Button>
        </section>

        <Dialog open={confirmOpen} onOpenChange={setConfirmOpen}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>确认覆盖当前渠道和模型</DialogTitle>
              <DialogDescription>
                该操作会清空当前设备上的所有渠道和模型，并使用导入文本重建。
              </DialogDescription>
            </DialogHeader>

            {preview && (
              <div className="space-y-4 text-sm">
                <div className="grid grid-cols-2 gap-3 rounded-md border p-3">
                  <div>
                    <p className="text-muted-foreground">即将导入</p>
                    <p>渠道：{preview.incomingChannels} 个</p>
                    <p>模型：{preview.incomingModels} 个</p>
                  </div>
                  <div>
                    <p className="text-muted-foreground">当前将被替换</p>
                    <p>渠道：{preview.currentChannels} 个</p>
                    <p>模型：{preview.currentModels} 个</p>
                  </div>
                </div>

                <div className="rounded-md border border-red-200 bg-red-50 p-3 text-red-700">
                  <p>不会迁移或覆盖：系统设置、模型池、日志、token、连接应用、代理端口、熔断/冷却/测速状态。</p>
                  <p>导入确认后不恢复旧渠道和旧模型；如导入失败，请重新粘贴有效导出文本再次导入。</p>
                </div>

                {preview.warnings.length > 0 && (
                  <div className="rounded-md border border-amber-200 bg-amber-50 p-3 text-amber-800">
                    <p className="font-medium">提示</p>
                    <ul className="mt-1 list-disc space-y-1 pl-5">
                      {preview.warnings.map((warning) => (
                        <li key={warning}>{warning}</li>
                      ))}
                    </ul>
                  </div>
                )}
              </div>
            )}

            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => setConfirmOpen(false)}
                disabled={importMutation.isPending}
              >
                取消
              </Button>
              <Button
                type="button"
                variant="destructive"
                onClick={confirmImport}
                disabled={importMutation.isPending}
              >
                {importMutation.isPending ? "正在导入..." : "确认覆盖当前渠道和模型"}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </CardContent>
    </Card>
  );
}
