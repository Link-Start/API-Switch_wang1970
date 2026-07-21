import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { ImageComposer } from "@/features/images/ImageComposer";
import { ImageConversation } from "@/features/images/ImageConversation";
import { blobFromResultSource, cloneImageReference, imageReferenceFromFile } from "@/features/images/imageInput";
import { getCatalogModel } from "@/lib/modelsCatalog";
import { buildImageRequest, buildProxyBaseUrl, callImagesEndpoint } from "@/features/images/requestBuilder";
import { addRecord, makeRecord, makeResult, restoreComposerFromRecord, setSelectedModel, setSubmitting, updateRecord, useImageStudioStore } from "@/features/images/imageStore";
import type { ImageEndpoint, ImageRecord, ImageResult } from "@/features/images/types";
import { useApiAdapter } from "@/lib/useApiAdapter";

type ImageEntryLike = {
  model: string;
  model_meta_zh?: string | null;
  model_meta_en?: string | null;
  display_name?: string;
};

// 生图模型判定：三路并集，任一命中即认为可生图
// 1) 模型名称含 "image"（如 gpt-image-1）
// 2) 目录结构化标签：models.json 的 modalities.output 含 "image"
// 3) 英文描述/标签（model_meta_en，目录 models.json 的原始规范标签；中文为翻译派生，不扫）
function isImageModel(entry: ImageEntryLike): boolean {
  const model = entry.model.trim().toLowerCase();
  if (!model) return false;
  if (model.includes("image")) return true;
  if (getCatalogModel(entry.model)?.modalities?.output?.includes("image")) return true;
  // ③ 以英文描述/标签为标准信号（CH/EN 两套中 EN 是规范标签，与目录 canonical 对齐；
  //    中文"生图"是翻译派生，不作独立判定词，避免中英文翻译差异导致的漏判/误判）
  const en = (entry.model_meta_en || "").toLowerCase();
  return /image gen|image generation|text[- ]?to[- ]?image|imagen/.test(en);
}

function entryModelsImageTag(entries: (ImageEntryLike & { enabled: boolean; channel_id: string })[], enabledChannelIds: Set<string>) {
  const seen = new Set<string>();
  const models: string[] = [];
  for (const entry of entries) {
    if (!entry.enabled || !enabledChannelIds.has(entry.channel_id)) continue;
    const model = entry.model.trim();
    if (!model || seen.has(model)) continue;
    if (!isImageModel(entry)) continue;
    seen.add(model);
    models.push(model);
  }
  return models;
}

export function ImageStudioPage() {
  const { t } = useTranslation();
  const api = useApiAdapter();
  const { composer, records } = useImageStudioStore();

  const { data: entries } = useQuery({
    queryKey: ["imageStudioEntries"],
    queryFn: () => api.pool.list(),
  });
  const { data: channels } = useQuery({
    queryKey: ["imageStudioChannels"],
    queryFn: () => api.channels.list(),
  });
  const { data: settings } = useQuery({ queryKey: ["settings"], queryFn: () => api.settings.get() });
  const { data: proxyStatus } = useQuery({ queryKey: ["proxyStatus"], queryFn: () => api.proxy.getStatus(), refetchInterval: 2000 });

  const enabledChannelIds = useMemo(() => new Set((channels || []).filter((channel) => channel.enabled).map((channel) => channel.id)), [channels]);
  const models = useMemo(() => entryModelsImageTag(entries || [], enabledChannelIds), [entries, enabledChannelIds]);
  useEffect(() => {
    if (models.length && !models.includes(composer.selectedModel)) setSelectedModel(models[0]);
  }, [models, composer.selectedModel]);

  const proxyBaseUrl = buildProxyBaseUrl(proxyStatus, settings);

  const submitRecord = async (record: ImageRecord, endpoint: ImageEndpoint, referenceImage?: ImageRecord["referenceImage"]) => {
    if (!proxyBaseUrl) {
      updateRecord(record.id, { status: "failed", errorSummary: t("imageStudio.proxyUnavailable") });
      toast.error(t("imageStudio.proxyUnavailable"));
      return;
    }
    setSubmitting(true);
    try {
      const payload = buildImageRequest({
        endpoint,
        model: record.model,
        prompt: record.prompt,
        size: record.size,
        count: record.count,
        referenceImage,
      });
      const results = await callImagesEndpoint(proxyBaseUrl, payload);
      const resultImages = results.map((result, index) => makeResult(record.id, index, result));
      updateRecord(record.id, { status: "succeeded", resultImages });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      updateRecord(record.id, { status: "failed", errorSummary: message || t("imageStudio.failed") });
      toast.error(t("imageStudio.failed"));
    } finally {
      setSubmitting(false);
    }
  };

  const handleSubmit = () => {
    if (!composer.selectedModel || !composer.prompt.trim() || composer.isSubmitting) return;
    const endpoint: ImageEndpoint = composer.referenceImage ? "edits" : "generations";
    const record = makeRecord({
      endpoint,
      model: composer.selectedModel,
      prompt: composer.prompt,
      size: composer.selectedSize,
      count: composer.selectedCount,
      referenceImage: cloneImageReference(composer.referenceImage),
    });
    addRecord(record);
    void submitRecord(record, endpoint, composer.referenceImage);
  };

  const handleRegenerate = (record: ImageRecord) => {
    if (composer.isSubmitting) return;
    restoreComposerFromRecord(record);
    const next = makeRecord({
      endpoint: record.endpoint,
      model: record.model,
      prompt: record.prompt,
      size: record.size,
      count: record.count,
      referenceImage: cloneImageReference(record.referenceImage),
      sourceRecordId: record.id,
    });
    addRecord(next);
    void submitRecord(next, record.endpoint, record.referenceImage);
  };

  const handleVariation = async (record: ImageRecord, result: ImageResult) => {
    if (composer.isSubmitting) return;
    if (!record.model) {
      toast.error(t("imageStudio.noModels"));
      return;
    }
    let reference;
    try {
      const blob = await blobFromResultSource(result);
      reference = imageReferenceFromFile(new File([blob], `variation-${result.index}.png`, { type: blob.type || "image/png" }));
    } catch {
      toast.error(t("imageStudio.remoteImageUnreadable"));
      return;
    }
    const next = makeRecord({
      endpoint: "variations",
      model: record.model,
      prompt: record.prompt,
      size: composer.selectedSize,
      count: composer.selectedCount,
      referenceImage: reference,
      sourceRecordId: record.id,
    });
    addRecord(next);
    void submitRecord(next, "variations", reference);
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 overflow-auto">
        <ImageConversation
          records={records}
          onRegenerate={handleRegenerate}
          onVariation={handleVariation}
        />
      </div>
      <div className="shrink-0 border-t bg-background p-3">
        <ImageComposer models={models} onSubmit={handleSubmit} />
      </div>
    </div>
  );
}

