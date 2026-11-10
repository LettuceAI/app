import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { SettingsWidgetNode } from "@/api/generated/bindings";
import { appAppearancePreferences, object, usePreferences } from "@/entities/preferences";
import { appearanceFields, type AppearanceDraft } from "@/features/groups";
import { characterAppearanceTarget, groupAppearanceTarget, type AppearanceSnapshot } from "../data/appearance-target";
import { fromSettingsNode, type WidgetSlots } from "./nodes";

export interface WidgetSource {
  characterId: string | null;
  groupId: string | null;
}

/**
 * The slots a chat shows: its own when the character or group overrides them, otherwise the app-wide ones, which are only ever read here.
 */
export function effectiveSlots(value: AppearanceDraft, preferences: Record<string, unknown>, group: boolean): WidgetSlots {
  if (appearanceFields(value, preferences).has("chat_widget_slots")) return value.presentation.chat_widget_slots;
  const global = object(appAppearancePreferences(preferences, group).chatWidgetSlots);
  const side = (nodes: unknown) => (Array.isArray(nodes) ? (nodes as SettingsWidgetNode[]).map(fromSettingsNode) : []);
  return { left: side(global.left), right: side(global.right) };
}

/**
 * The widget slots of the open chat, read from the same appearance snapshot the appearance drawer edits, and a `save` that writes a change back to the character or group. Saves run one after another; a revision conflict reloads the snapshot and applies the change once more.
 */
export function useWidgetSlots(api: ApiClient, source: WidgetSource, enabled: boolean) {
  const { t } = useTranslation("common");
  const queryClient = useQueryClient();
  const preferences = usePreferences();
  const latest = useRef(preferences.value);
  useEffect(() => {
    latest.current = preferences.value;
  }, [preferences.value]);
  const group = source.groupId !== null;
  const target = useMemo(
    () =>
      source.groupId
        ? groupAppearanceTarget(api, source.groupId)
        : source.characterId
          ? characterAppearanceTarget(api, source.characterId)
          : null,
    [api, source.groupId, source.characterId],
  );
  const query = useQuery({
    queryKey: target?.key ?? ["widget-slots", "none"],
    queryFn: () => (target ? target.load() : Promise.reject(new Error("no appearance target"))),
    enabled: enabled && target !== null,
  });
  const [picked, setPicked] = useState<ReadonlyMap<string, string>>(new Map());
  const chain = useRef<Promise<unknown>>(Promise.resolve());

  const slots = useMemo(
    () => (query.data && preferences.ready ? effectiveSlots(query.data.value, preferences.value, group) : null),
    [query.data, preferences.ready, preferences.value, group],
  );
  const assets = useMemo(
    () => new Map<string, string>([...(query.data?.assets ?? []).map((asset) => [asset.asset_id, asset.url] as const), ...picked]),
    [query.data, picked],
  );
  const remember = useCallback((assetId: string, url: string) => {
    setPicked((current) => new Map(current).set(assetId, url));
  }, []);

  const save = useCallback(
    (mutate: (slots: WidgetSlots) => WidgetSlots): Promise<boolean> => {
      if (!target) return Promise.resolve(false);
      const attempt = async (retried: boolean): Promise<boolean> => {
        try {
          const snapshot: AppearanceSnapshot =
            queryClient.getQueryData<AppearanceSnapshot>(target.key) ?? (await queryClient.fetchQuery<AppearanceSnapshot>({ queryKey: target.key, queryFn: target.load }));
          const fields = appearanceFields(snapshot.value, latest.current);
          fields.add("chat_widget_slots");
          const next: AppearanceDraft = {
            presentation: {
              ...snapshot.value.presentation,
              chat_widget_slots: mutate(effectiveSlots(snapshot.value, latest.current, group)),
            },
            presentation_overrides: [...fields].sort(),
          };
          queryClient.setQueryData(target.key, await snapshot.save(next, crypto.randomUUID()));
          return true;
        } catch (error) {
          if (!retried && isApiFailure(error) && error.code === "conflict") {
            await queryClient.fetchQuery({ queryKey: target.key, queryFn: target.load, staleTime: 0 }).catch(() => undefined);
            return attempt(true);
          }
          toast.error(t(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
          return false;
        }
      };
      const run = chain.current.then(() => attempt(false));
      chain.current = run;
      return run;
    },
    [target, queryClient, group, t],
  );

  const { refetch } = query;
  const retry = useCallback(() => void refetch(), [refetch]);
  return { slots, assets, remember, save, failure: query.isError ? { error: query.error, retry } : null };
}
