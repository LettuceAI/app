import { useCallback, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { JobPage, JobView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { terminalJobStates } from "@/entities/job";
import { useSettingsApi } from "../data";

export function useFolderMoves() {
  const api = useSettingsApi();
  const [admitted, setAdmitted] = useState<string | null>(null);
  const [observed, setObserved] = useState<Record<string, JobView>>({});
  const query = useQuery({
    queryKey: [...queryKeys.jobs.lists(), "models-folder-moves"],
    queryFn: async () => {
      const items: JobView[] = [];
      let cursor: string | null = null;
      do {
        const page: JobPage = await api.call("jobsList", {
          kinds: ["models_folder_move"],
          states: [],
          subject: null,
          cursor,
          limit: 100,
        });
        items.push(...page.items);
        cursor = page.next_cursor;
      } while (cursor);
      return items;
    },
  });
  const jobs = new Map((query.data ?? []).map((job) => [job.id, job]));
  for (const job of Object.values(observed)) {
    const seeded = jobs.get(job.id);
    if (!seeded || job.updated_at >= seeded.updated_at) jobs.set(job.id, job);
  }
  const ids = new Set(
    [...jobs.values()]
      .filter(
        (job) =>
          !terminalJobStates.has(job.state) ||
          ["failed", "interrupted", "cancelled"].includes(job.state),
      )
      .map((job) => job.id),
  );
  if (admitted) ids.add(admitted);
  const active =
    query.isPending ||
    query.isError ||
    [...jobs.values()].some((job) => !terminalJobStates.has(job.state)) ||
    (admitted !== null && !jobs.has(admitted));
  const observe = useCallback(
    (job: JobView) => setObserved((current) => ({ ...current, [job.id]: job })),
    [],
  );
  return {
    active,
    admit: setAdmitted,
    ids: [...ids],
    jobs,
    observe,
    error: query.error,
    retry: () => void query.refetch(),
  };
}
