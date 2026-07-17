import { useSyncExternalStore } from "react";
import type { ImageComposerState, ImageCount, ImageRecord, ImageReference, ImageResult, ImageSize, ImageStudioSnapshot } from "./types";
import { revokeImageReference } from "./imageInput";

const initialComposer: ImageComposerState = {
  prompt: "",
  selectedModel: "",
  selectedSize: "1024x1024",
  selectedCount: 1,
  isSubmitting: false,
  compositionActive: false,
};

let snapshot: ImageStudioSnapshot = { composer: initialComposer, records: [] };
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((listener) => listener());
}

function update(mutator: (draft: ImageStudioSnapshot) => ImageStudioSnapshot) {
  snapshot = mutator(snapshot);
  emit();
}

export function useImageStudioStore() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => snapshot,
    () => snapshot,
  );
}

export function setPrompt(prompt: string) {
  update((current) => ({ ...current, composer: { ...current.composer, prompt } }));
}

export function setSelectedModel(selectedModel: string) {
  update((current) => ({ ...current, composer: { ...current.composer, selectedModel } }));
}

export function setSelectedSize(selectedSize: ImageSize) {
  update((current) => ({ ...current, composer: { ...current.composer, selectedSize } }));
}

export function setSelectedCount(selectedCount: ImageCount) {
  update((current) => ({ ...current, composer: { ...current.composer, selectedCount } }));
}

export function setCompositionActive(compositionActive: boolean) {
  update((current) => ({ ...current, composer: { ...current.composer, compositionActive } }));
}

export function setSubmitting(isSubmitting: boolean) {
  update((current) => ({ ...current, composer: { ...current.composer, isSubmitting } }));
}

export function setReferenceImage(referenceImage?: ImageReference) {
  update((current) => {
    revokeImageReference(current.composer.referenceImage);
    return { ...current, composer: { ...current.composer, referenceImage } };
  });
}

export function restoreComposerFromRecord(record: ImageRecord) {
  update((current) => ({
    ...current,
    composer: {
      ...current.composer,
      prompt: record.prompt,
      selectedModel: record.model,
      selectedSize: record.size,
      selectedCount: record.count,
      referenceImage: record.referenceImage,
    },
  }));
}

export function addRecord(record: ImageRecord) {
  update((current) => ({ ...current, records: [record, ...current.records] }));
}

export function updateRecord(id: string, patch: Partial<ImageRecord>) {
  update((current) => ({
    ...current,
    records: current.records.map((record) => (record.id === id ? { ...record, ...patch } : record)),
  }));
}

export function makeRecord(params: Omit<ImageRecord, "id" | "createdAt" | "status" | "resultImages">): ImageRecord {
  return {
    ...params,
    id: crypto.randomUUID(),
    createdAt: Date.now(),
    status: "pending",
    resultImages: [],
  };
}

export function makeResult(recordId: string, index: number, result: Omit<ImageResult, "id" | "recordId" | "index">): ImageResult {
  return { ...result, id: crypto.randomUUID(), recordId, index };
}
