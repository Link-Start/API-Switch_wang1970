import { useState } from "react";
import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation();
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
      toast.success(t("settings.importExport.toast.exportSuccess"));
    },
    onError: (error) => {
      toast.error(t("settings.importExport.toast.exportFailed", { error }));
    },
  });

  const previewMutation = useMutation({
    mutationFn: (payload: string) => adapter.importExport.previewChannelModel(payload),
    onSuccess: (data) => {
      setPreview(data);
      setConfirmOpen(true);
    },
    onError: (error) => {
      toast.error(t("settings.importExport.toast.previewFailed", { error }));
    },
  });

  const importMutation = useMutation({
    mutationFn: (payload: string) => adapter.importExport.importChannelModel(payload),
    onSuccess: (result) => {
      toast.success(
        t("settings.importExport.toast.importSuccess", {
          channels: result.channelCount,
          models: result.modelCount,
        }),
      );
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
      toast.error(t("settings.importExport.toast.importFailed", { error }));
    },
  });

  const isBusy = exportMutation.isPending || previewMutation.isPending || importMutation.isPending;

  const copyExportText = async () => {
    if (!exportText) return;
    try {
      await navigator.clipboard.writeText(exportText);
      toast.success(t("settings.importExport.toast.copySuccess"));
    } catch {
      toast.error(t("settings.importExport.toast.copyFailed"));
    }
  };

  const startImport = () => {
    const payload = importText.trim();
    if (!payload) {
      toast.error(t("settings.importExport.toast.emptyImport"));
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
        <CardTitle className="text-base">{t("settings.importExport.title")}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-6">
        <div className="rounded-md border border-amber-200 bg-amber-50 p-3 text-sm text-amber-800">
          <div className="flex gap-2">
            <AlertTriangle className="mt-0.5 h-4 w-4 flex-shrink-0" />
            <div className="space-y-1">
              <p>{t("settings.importExport.warningContent1")}</p>
              <p>{t("settings.importExport.warningContent2")}</p>
              <p>{t("settings.importExport.warningContent3")}</p>
            </div>
          </div>
        </div>

        <section className="space-y-3">
          <div>
            <h3 className="font-medium">{t("settings.importExport.exportSectionTitle")}</h3>
            <p className="text-sm text-muted-foreground">
              {t("settings.importExport.exportSectionDesc")}
            </p>
          </div>
          <div className="flex gap-2">
            <Button
              type="button"
              onClick={() => exportMutation.mutate()}
              disabled={isBusy}
            >
              {exportMutation.isPending ? t("settings.importExport.exporting") : t("settings.importExport.exportButton")}
            </Button>
            {exportText && (
              <Button type="button" variant="outline" onClick={copyExportText}>
                <Copy className="mr-2 h-4 w-4" />
                {t("settings.importExport.copyButton")}
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
            <h3 className="font-medium">{t("settings.importExport.importSectionTitle")}</h3>
            <p className="text-sm text-muted-foreground">
              {t("settings.importExport.importSectionDesc")}
            </p>
          </div>
          <textarea
            value={importText}
            onChange={(event) => setImportText(event.target.value)}
            rows={8}
            placeholder={t("settings.importExport.importPlaceholder")}
            className="w-full rounded-md border bg-background p-3 font-mono text-xs"
            disabled={isBusy}
          />
          <Button
            type="button"
            variant="destructive"
            onClick={startImport}
            disabled={!importText.trim() || isBusy}
          >
            {previewMutation.isPending ? t("settings.importExport.validating") : t("settings.importExport.importButton")}
          </Button>
        </section>

        <Dialog open={confirmOpen} onOpenChange={setConfirmOpen}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>{t("settings.importExport.dialogTitle")}</DialogTitle>
              <DialogDescription>
                {t("settings.importExport.dialogDesc")}
              </DialogDescription>
            </DialogHeader>

            {preview && (
              <div className="space-y-4 text-sm">
                <div className="grid grid-cols-2 gap-3 rounded-md border p-3">
                  <div>
                    <p className="text-muted-foreground">{t("settings.importExport.incomingData")}</p>
                    <p>{t("settings.importExport.channelsCount", { count: preview.incomingChannels })}</p>
                    <p>{t("settings.importExport.modelsCount", { count: preview.incomingModels })}</p>
                  </div>
                  <div>
                    <p className="text-muted-foreground">{t("settings.importExport.currentData")}</p>
                    <p>{t("settings.importExport.channelsCount", { count: preview.currentChannels })}</p>
                    <p>{t("settings.importExport.modelsCount", { count: preview.currentModels })}</p>
                  </div>
                </div>

                <div className="rounded-md border border-red-200 bg-red-50 p-3 text-red-700">
                  <p>{t("settings.importExport.dialogWarning1")}</p>
                  <p>{t("settings.importExport.dialogWarning2")}</p>
                </div>

                {preview.warnings.length > 0 && (
                  <div className="rounded-md border border-amber-200 bg-amber-50 p-3 text-amber-800">
                    <p className="font-medium">{t("settings.importExport.warningTitle")}</p>
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
                {t("common.cancel")}
              </Button>
              <Button
                type="button"
                variant="destructive"
                onClick={confirmImport}
                disabled={importMutation.isPending}
              >
                {importMutation.isPending ? t("settings.importExport.importing") : t("settings.importExport.confirmButton")}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </CardContent>
    </Card>
  );
}
