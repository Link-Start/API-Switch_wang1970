import type { ImageReference } from "./types";

export function revokeImageReference(reference?: ImageReference) {
  if (reference?.objectUrl) URL.revokeObjectURL(reference.objectUrl);
}

export function imageReferenceFromFile(file: File): ImageReference {
  return {
    blob: file,
    objectUrl: URL.createObjectURL(file),
    name: file.name || "image",
    size: file.size,
    type: file.type || "application/octet-stream",
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

export async function downloadImage(source: { b64Json?: string; objectUrl?: string; url?: string; mime?: string }, filename: string) {
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
  } catch {
    if (source.url) {
      window.open(source.url, "_blank", "noopener,noreferrer");
      return;
    }
    throw new Error("download_failed");
  }
}
