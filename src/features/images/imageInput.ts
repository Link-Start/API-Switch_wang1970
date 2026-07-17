import type { ImageReference } from "./types";

export function revokeImageReference(reference?: ImageReference) {
  if (reference?.objectUrl) URL.revokeObjectURL(reference.objectUrl);
}

function extensionForMime(type: string) {
  if (type === "image/jpeg") return ".jpg";
  if (type === "image/png") return ".png";
  if (type === "image/webp") return ".webp";
  return "";
}

function normalizeImageFilename(name: string | undefined, type: string) {
  const fallback = `image${extensionForMime(type) || ".png"}`;
  const trimmed = name?.trim() || fallback;
  return /\.[a-z0-9]+$/i.test(trimmed) ? trimmed : `${trimmed}${extensionForMime(type)}`;
}

export function imageReferenceFromFile(file: File): ImageReference {
  const type = file.type || "image/png";
  return {
    blob: file,
    objectUrl: URL.createObjectURL(file),
    name: normalizeImageFilename(file.name, type),
    size: file.size,
    type,
  };
}

export function getFirstImageFile(files?: FileList | File[] | null): File | null {
  if (!files) return null;
  const list = Array.from(files as ArrayLike<File>);
  return list.find((file) => file.type.startsWith("image/")) || list[0] || null;
}

export function getImageFileFromClipboard(event: React.ClipboardEvent): File | null {
  const items = Array.from(event.clipboardData?.items || []);
  for (const item of items) {
    if (item.kind === "file") {
      const file = item.getAsFile();
      if (file) return file;
    }
  }
  return getFirstImageFile(event.clipboardData?.files || null);
}

export function getImageFileFromDrop(event: React.DragEvent): File | null {
  return getFirstImageFile(event.dataTransfer?.files || null);
}

export async function blobFromResultSource(source: { b64Json?: string; objectUrl?: string; url?: string; mime?: string }): Promise<Blob> {
  if (source.b64Json) {
    const binary = atob(source.b64Json);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
    return new Blob([bytes], { type: source.mime || "image/png" });
  }
  const url = source.objectUrl || source.url;
  if (!url) throw new Error("No readable image source");
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Image fetch failed: ${response.status}`);
  return response.blob();
}

export type ImageDownloadResult = "downloaded" | "opened-original";

export async function downloadImage(source: { b64Json?: string; objectUrl?: string; url?: string; mime?: string }, filename: string): Promise<ImageDownloadResult> {
  try {
    const blob = await blobFromResultSource(source);
    const objectUrl = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = objectUrl;
    link.download = filename;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(objectUrl);
    return "downloaded";
  } catch {
    if (source.url) {
      const opened = window.open(source.url, "_blank", "noopener,noreferrer");
      if (opened) return "opened-original";
    }
    throw new Error("download_failed");
  }
}
