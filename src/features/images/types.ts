export type ImageEndpoint = "generations" | "edits" | "variations";
export type ImageSize = "1024x1024" | "1536x1024" | "1024x1536";
export type ImageCount = 1 | 2 | 4;
export type ImageRecordStatus = "pending" | "succeeded" | "failed";

export interface ImageReference {
  blob: Blob;
  objectUrl: string;
  name: string;
  size: number;
  type: string;
}

export interface ImageResult {
  id: string;
  recordId: string;
  index: number;
  url?: string;
  proxiedUrl?: string;
  b64Json?: string;
  objectUrl?: string;
  mime?: string;
  loadError?: string;
}

export interface ImageRecord {
  id: string;
  createdAt: number;
  endpoint: ImageEndpoint;
  status: ImageRecordStatus;
  model: string;
  prompt: string;
  size: ImageSize;
  count: ImageCount;
  referenceImage?: ImageReference;
  sourceRecordId?: string;
  resultImages: ImageResult[];
  errorSummary?: string;
}

export interface ImageComposerState {
  prompt: string;
  selectedModel: string;
  selectedSize: ImageSize;
  selectedCount: ImageCount;
  referenceImage?: ImageReference;
  isSubmitting: boolean;
  compositionActive: boolean;
}

export interface ImageStudioSnapshot {
  composer: ImageComposerState;
  records: ImageRecord[];
}

export const IMAGE_SIZES: { value: ImageSize; labelKey: string }[] = [
  { value: "1024x1024", labelKey: "imageStudio.size.square" },
  { value: "1536x1024", labelKey: "imageStudio.size.landscape" },
  { value: "1024x1536", labelKey: "imageStudio.size.portrait" },
];

export const IMAGE_COUNTS: ImageCount[] = [1, 2, 4];
