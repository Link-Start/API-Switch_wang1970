import { RefreshCw, Sparkles } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import type { ImageRecord, ImageResult } from "./types";

interface ImageResultCardProps {
  record: ImageRecord;
  result: ImageResult;
  onRegenerate: (record: ImageRecord) => void;
  onVariation: (record: ImageRecord, result: ImageResult) => void;
}

function aspectRatioForSize(size: string) {
  if (size === "1536x1024") return "3 / 2";
  if (size === "1024x1536") return "2 / 3";
  return "1 / 1";
}

export function ImageResultCard({ record, result, onRegenerate, onVariation }: ImageResultCardProps) {
  const { t } = useTranslation();
  const [loadError, setLoadError] = useState(false);
  const src = result.objectUrl || result.url;
  const usable = !loadError && !!src;

  return (
    <div className="flex flex-col gap-2">
      <div className="overflow-hidden rounded-xl border bg-muted" style={{ aspectRatio: aspectRatioForSize(record.size) }}>
        {usable ? (
          <img src={src} alt={record.prompt} className="h-full w-full object-cover" onError={() => setLoadError(true)} />
        ) : (
          <div className="flex h-full w-full items-center justify-center text-xs text-muted-foreground">
            {t("imageStudio.imageLoadFailed")}
          </div>
        )}
      </div>
      <div className="flex flex-wrap items-center justify-end gap-1">
        <Button type="button" variant="ghost" size="sm" onClick={() => onRegenerate(record)} title={t("imageStudio.regenerate")} aria-label={t("imageStudio.regenerate")}>
          <RefreshCw className="h-4 w-4" />
        </Button>
        <Button type="button" variant="ghost" size="sm" onClick={() => onVariation(record, result)} title={t("imageStudio.variation")} aria-label={t("imageStudio.variation")}>
          <Sparkles className="h-4 w-4" />
        </Button>
      </div>
      {record.count > 1 && <span className="text-right text-xs text-muted-foreground">{result.index + 1}/{record.count}</span>}
    </div>
  );
}
