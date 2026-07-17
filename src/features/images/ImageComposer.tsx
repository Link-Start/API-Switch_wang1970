import { Paperclip, Send, X } from "lucide-react";
import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { ImageCount, ImageSize } from "./types";
import { IMAGE_COUNTS, IMAGE_SIZES } from "./types";
import { getImageFileFromClipboard, getImageFileFromDrop, imageReferenceFromFile } from "./imageInput";
import { setCompositionActive, setPrompt, setReferenceImage, setSelectedCount, setSelectedModel, setSelectedSize, useImageStudioStore } from "./imageStore";

interface ImageComposerProps {
  models: string[];
  onSubmit: () => void;
  disabled?: boolean;
}

export function ImageComposer({ models, onSubmit, disabled }: ImageComposerProps) {
  const { t } = useTranslation();
  const { composer } = useImageStudioStore();
  const fileRef = useRef<HTMLInputElement>(null);

  const chooseFile = (file: File | null) => {
    if (!file) return;
    setReferenceImage(imageReferenceFromFile(file));
  };

  const submitDisabled = disabled || composer.isSubmitting || !composer.prompt.trim() || !composer.selectedModel;

  return (
    <div
      className="rounded-2xl border bg-background p-3 shadow-lg"
      onDragOver={(event) => {
        event.preventDefault();
        event.dataTransfer.dropEffect = "copy";
      }}
      onDrop={(event) => {
        event.preventDefault();
        chooseFile(getImageFileFromDrop(event));
      }}
      onPaste={(event) => chooseFile(getImageFileFromClipboard(event))}
    >
      {composer.referenceImage && (
        <div className="mb-2 flex items-center gap-2 rounded-lg bg-muted p-2 text-xs text-muted-foreground">
          <img src={composer.referenceImage.objectUrl} alt="reference" className="h-12 w-12 rounded object-cover" />
          <span className="min-w-0 flex-1 truncate">{composer.referenceImage.name}</span>
          <Button type="button" variant="ghost" size="icon" className="h-8 w-8" onClick={() => setReferenceImage(undefined)}>
            <X className="h-4 w-4" />
          </Button>
        </div>
      )}
      <textarea
        value={composer.prompt}
        onChange={(event) => setPrompt(event.target.value)}
        onCompositionStart={() => setCompositionActive(true)}
        onCompositionEnd={() => setCompositionActive(false)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey && !composer.compositionActive) {
            event.preventDefault();
            if (!submitDisabled) onSubmit();
          }
        }}
        placeholder={t("imageStudio.promptPlaceholder")}
        className="min-h-24 w-full resize-none rounded-lg border bg-background p-3 text-sm outline-none focus:ring-2 focus:ring-ring"
      />
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <input ref={fileRef} type="file" accept="image/*" className="hidden" onChange={(event) => chooseFile(event.target.files?.[0] || null)} />
        <Button type="button" variant="outline" size="sm" onClick={() => fileRef.current?.click()}>
          <Paperclip className="mr-1 h-4 w-4" />
          {t("imageStudio.addImage")}
        </Button>
        <Select value={composer.selectedModel} onValueChange={setSelectedModel}>
          <SelectTrigger className="w-52">
            <SelectValue placeholder={t("imageStudio.selectModel")} />
          </SelectTrigger>
          <SelectContent>
            {models.map((model) => <SelectItem key={model} value={model}>{model}</SelectItem>)}
          </SelectContent>
        </Select>
        <Select value={composer.selectedSize} onValueChange={(value) => setSelectedSize(value as ImageSize)}>
          <SelectTrigger className="w-32"><SelectValue /></SelectTrigger>
          <SelectContent>
            {IMAGE_SIZES.map((size) => <SelectItem key={size.value} value={size.value}>{t(size.labelKey)}</SelectItem>)}
          </SelectContent>
        </Select>
        <Select value={String(composer.selectedCount)} onValueChange={(value) => setSelectedCount(Number(value) as ImageCount)}>
          <SelectTrigger className="w-24"><SelectValue /></SelectTrigger>
          <SelectContent>
            {IMAGE_COUNTS.map((count) => <SelectItem key={count} value={String(count)}>{count}</SelectItem>)}
          </SelectContent>
        </Select>
        <Button type="button" className="ml-auto" disabled={submitDisabled} onClick={onSubmit}>
          <Send className="mr-1 h-4 w-4" />
          {composer.isSubmitting ? t("imageStudio.generating") : t("imageStudio.generate")}
        </Button>
      </div>
      {models.length === 0 && <div className="mt-2 text-xs text-muted-foreground">{t("imageStudio.noModels")}</div>}
    </div>
  );
}
