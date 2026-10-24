import { ApiFailure } from "./client";

export class AudioSource {
  private readonly source: string;
  private url: string | null = null;
  private pending: Promise<string> | null = null;
  private controller: AbortController | null = null;

  constructor(source: string) { this.source = source; }

  load(): Promise<string> {
    if (!this.source.startsWith("lettuce-asset:")) return Promise.resolve(this.source);
    if (this.url) return Promise.resolve(this.url);
    if (this.pending) return this.pending;
    const controller = new AbortController();
    this.controller = controller;
    this.pending = this.read(controller).finally(() => {
      if (this.controller === controller) { this.controller = null; this.pending = null; }
    });
    return this.pending;
  }

  private async read(controller: AbortController): Promise<string> {
    try {
      const response = await fetch(this.source, { signal: controller.signal });
      if (!response.ok) throw new ApiFailure(response.status === 404 ? "not_found" : "unavailable", "audio asset could not be loaded", null);
      const blob = await response.blob();
      controller.signal.throwIfAborted();
      this.url = URL.createObjectURL(blob);
      return this.url;
    } catch (error) {
      if (controller.signal.aborted || error instanceof ApiFailure) throw error;
      throw new ApiFailure("unavailable", "audio asset could not be loaded", null);
    }
  }

  release() {
    this.controller?.abort();
    this.controller = null;
    this.pending = null;
    if (this.url) URL.revokeObjectURL(this.url);
    this.url = null;
  }
}
