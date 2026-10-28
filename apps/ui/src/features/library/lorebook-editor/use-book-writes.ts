import { useCallback, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import { ApiFailure, isApiFailure } from "@/api/client";
import type {
  LorebookEntryInput,
  LorebookEntryMutationInput,
  LorebookEntryView,
  LorebookView_Serialize,
} from "@/api/generated/bindings";
import { useApi } from "@/api/use-api";
import { lorebookBookKey, lorebookBookQuery } from "./data";

type Overlay = {
  id: string;
  apply: (entries: LorebookEntryView[]) => LorebookEntryView[];
  after: number | null;
};

export function inputOf(entry: LorebookEntryView): LorebookEntryInput {
  return {
    title: entry.title,
    enabled: entry.enabled,
    always_active: entry.always_active,
    keywords: entry.keywords,
    case_sensitive: entry.case_sensitive,
    keyword_mode: entry.keyword_mode,
    content: entry.content,
    priority: entry.priority,
  };
}

export function useBookWrites(lorebookId: string, book: LorebookView_Serialize | undefined) {
  const api = useApi();
  const client = useQueryClient();
  const { t } = useTranslation();
  const key = useMemo(() => lorebookBookKey(lorebookId), [lorebookId]);
  const tail = useRef<Promise<unknown>>(Promise.resolve());
  const own = useRef(0);
  const [overlays, setOverlays] = useState<Overlay[]>([]);
  const revision = book?.lorebook.revision ?? 0;
  const settled = overlays.filter((overlay) => overlay.after !== null && overlay.after <= revision);
  if (settled.length > 0)
    setOverlays((list) =>
      list.filter((overlay) => !settled.some((done) => done.id === overlay.id)),
    );
  const live = overlays.filter((overlay) => !settled.includes(overlay));

  const noteRevision = useCallback((value: number) => {
    own.current = Math.max(own.current, value);
  }, []);
  const ownRevision = useCallback(() => own.current, []);

  const submit = useCallback(
    <T>(task: (revision: number) => Promise<T>): Promise<T> => {
      const run = tail.current.then(() => {
        const current = client.getQueryData<LorebookView_Serialize>(key);
        if (!current) throw new ApiFailure("internal", "lorebook is not loaded", null);
        return task(Math.max(own.current, current.lorebook.revision));
      });
      tail.current = run.then(
        () => undefined,
        async (error: unknown) => {
          if (isApiFailure(error) && error.code === "conflict")
            await client
              .fetchQuery({ ...lorebookBookQuery(api, lorebookId), staleTime: 0 })
              .catch(() => undefined);
        },
      );
      return run;
    },
    [api, client, key, lorebookId],
  );

  const drain = useCallback(() => tail.current, []);

  const fail = useCallback(
    (error: unknown) => {
      const code = isApiFailure(error) ? error.code : "internal";
      toast.error(t(`errors.code.${code}`));
    },
    [t],
  );

  const mutate = useCallback(
    async (mutation: LorebookEntryMutationInput, apply: Overlay["apply"]): Promise<boolean> => {
      const id = crypto.randomUUID();
      setOverlays((list) => [...list, { id, apply, after: null }]);
      try {
        const view = await submit((expected) =>
          api.call("lorebookEntriesMutate", {
            lorebook_id: lorebookId,
            expected_revision: expected,
            client_operation_id: id,
            mutations: [mutation],
          }),
        );
        noteRevision(view.lorebook.revision);
        client.setQueryData(key, view);
        setOverlays((list) =>
          list.map((overlay) =>
            overlay.id === id ? { ...overlay, after: view.lorebook.revision } : overlay,
          ),
        );
        return true;
      } catch (error) {
        setOverlays((list) => list.filter((overlay) => overlay.id !== id));
        fail(error);
        return false;
      }
    },
    [api, client, fail, key, lorebookId, noteRevision, submit],
  );

  const entries = book
    ? live.reduce((list, overlay) => overlay.apply(list), book.entries)
    : undefined;
  return { entries, submit, mutate, fail, drain, noteRevision, ownRevision };
}
