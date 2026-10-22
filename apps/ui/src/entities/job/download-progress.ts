import type { JobView } from "@/api/generated/bindings";

export function downloadProgressOf(job: JobView | null): { fraction: number | undefined; size: number | undefined; speed: number | undefined; eta: number | undefined } {
  const progress = job?.progress;
  if (!progress || progress.unit !== "bytes" || !progress.total) {
    return { fraction: undefined, size: undefined, speed: undefined, eta: undefined };
  }
  const speed = progress.bytes_per_second && progress.bytes_per_second > 0 ? progress.bytes_per_second : undefined;
  return {
    fraction: Math.min(1, progress.current / progress.total),
    size: progress.total,
    speed,
    eta: speed ? Math.max(0, (progress.total - progress.current) / speed) : undefined,
  };
}
