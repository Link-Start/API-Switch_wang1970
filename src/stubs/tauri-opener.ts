// Stub for @tauri-apps/plugin-opener — web mode uses window.open
export async function openUrl(url: string): Promise<void> {
  window.open(url, "_blank", "noopener,noreferrer");
}
