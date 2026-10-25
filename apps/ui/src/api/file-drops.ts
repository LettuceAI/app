export async function listenFileDrops(
  onDrop: (paths: string[], position: { x: number; y: number }) => void,
) {
  const { getCurrentWebview } = await import("@tauri-apps/api/webview");
  return getCurrentWebview().onDragDropEvent(({ payload }) => {
    if (payload.type === "drop") onDrop(payload.paths, payload.position);
  });
}
