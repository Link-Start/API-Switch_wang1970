import { useTranslation } from "react-i18next";
import type { ImageRecord, ImageResult } from "./types";
import { ImageResultCard } from "./ImageResultCard";

interface ImageConversationProps {
  records: ImageRecord[];
  selectedSize: string;
  selectedCount: number;
  onRegenerate: (record: ImageRecord) => void;
  onVariation: (record: ImageRecord, result: ImageResult) => void;
}

function StatusPill({ status }: { status: ImageRecord["status"] }) {
  const { t } = useTranslation();
  const label = status === "pending" ? t("imageStudio.statusPending") : status === "succeeded" ? t("imageStudio.statusSucceeded") : t("imageStudio.statusFailed");
  return <span className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">{label}</span>;
}

export function ImageConversation({ records, selectedSize, selectedCount, onRegenerate, onVariation }: ImageConversationProps) {
  const { t } = useTranslation();
  if (records.length === 0) {
    return <div className="flex h-full items-center justify-center p-6 text-center text-sm text-muted-foreground">{t("imageStudio.empty")}</div>;
  }
  return (
    <div className="space-y-4 p-4">
      {records.map((record) => (
        <div key={record.id} className="rounded-2xl border bg-card p-3 shadow-sm">
          <div className="mb-2 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <StatusPill status={record.status} />
            <span className="truncate font-medium text-foreground">{record.model}</span>
            <span>{record.size}</span>
            <span>x{record.count}</span>
            <span>{record.endpoint}</span>
            {record.referenceImage && <img src={record.referenceImage.objectUrl} alt="reference" className="h-6 w-6 rounded object-cover" />}
          </div>
          <p className="mb-2 whitespace-pre-wrap break-words text-sm">{record.prompt}</p>
          {record.errorSummary && <div className="mb-2 rounded-lg bg-destructive/10 p-2 text-xs text-destructive">{record.errorSummary}</div>}
          <div className="grid gap-3" style={{ gridTemplateColumns: `repeat(${Math.min(record.resultImages.length || 1, 2)}, minmax(0, 1fr))` }}>
            {record.resultImages.map((result) => (
              <ImageResultCard key={result.id} record={record} result={result} onRegenerate={onRegenerate} onVariation={onVariation} selectedSize={selectedSize} selectedCount={selectedCount} />
            ))}
            {record.status === "pending" && (
              <div className="overflow-hidden rounded-xl border bg-muted" style={{ aspectRatio: selectedSize === "1024x1024" ? "1 / 1" : selectedSize === "1536x1024" ? "3 / 2" : "2 / 3" }} />
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
