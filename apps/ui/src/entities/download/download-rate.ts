import type { JobView } from "@/api/generated/bindings";

type Sample = { current: number; total: number | null; stage: string | null; at: number; speed: number | null };

export class DownloadRateTracker {
  private readonly samples = new Map<string, Sample>();

  has(id: string) { return this.samples.has(id); }

  observe(job: JobView, at: number): JobView {
    const progress = job.progress;
    if (job.state !== "running" || progress.unit !== "bytes" || progress.label_code !== null && progress.label_code !== "download") {
      this.samples.delete(job.id);
      return { ...job, progress: { ...progress, bytes_per_second: null } };
    }
    const previous = this.samples.get(job.id);
    let speed = progress.bytes_per_second !== null && progress.bytes_per_second > 0 ? progress.bytes_per_second : null;
    if (previous && previous.total === progress.total && previous.stage === progress.label_code && progress.current >= previous.current) {
      speed = previous.speed ?? speed;
      if (progress.current === previous.current || at <= previous.at) return { ...job, progress: { ...progress, bytes_per_second: speed } };
      speed = (progress.current - previous.current) * 1000 / (at - previous.at);
    } else if (previous) {
      speed = null;
    }
    this.samples.set(job.id, { current: progress.current, total: progress.total, stage: progress.label_code, at, speed });
    return { ...job, progress: { ...progress, bytes_per_second: speed } };
  }
}
