import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useBlocker } from "@tanstack/react-router";
import { PanelLeftOpen, PanelRightOpen, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, cn, confirm, icon, IconButton, LoadingState, motion as presets, useCompactLayout } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import { usePreferences, type ChatLook } from "@/entities/preferences";
import { AppearanceEditor, appearanceLook, type AppearanceDraft } from "@/features/groups";
import { FailureState } from "@/shared/ui/FailureState";
import { characterAppearanceTarget, groupAppearanceTarget } from "../../data/appearance-target";
import { readSide, writeSide, type DrawerSide } from "./drawer-layout";

export interface ChatAppearanceDrawerProps {
  open: boolean;
  api: ApiClient;
  characterId: string | null;
  groupId: string | null;
  name: string;
  onClose: () => void;
  onDraft: (look: ChatLook | null) => void;
}

export function ChatAppearanceDrawer(props: ChatAppearanceDrawerProps) {
  const target = props.groupId ?? props.characterId;
  return createPortal(<AnimatePresence>{props.open && target !== null && <DrawerBody key="appearance" {...props} />}</AnimatePresence>, document.body);
}

function DrawerBody({ api, characterId, groupId, name, onClose, onDraft }: ChatAppearanceDrawerProps) {
  const { t } = useTranslation("conversations");
  const queryClient = useQueryClient();
  const phone = useCompactLayout();
  const preferences = usePreferences();
  const target = useMemo(() => (groupId ? groupAppearanceTarget(api, groupId) : characterAppearanceTarget(api, characterId ?? "")), [api, groupId, characterId]);
  const group = target.kind === "group";
  const query = useQuery({ queryKey: target.key, queryFn: target.load, gcTime: 0, staleTime: 0, refetchOnMount: "always" });
  const [draft, setDraft] = useState<AppearanceDraft | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [saving, setSaving] = useState(false);
  const attempt = useRef<{ payload: string; id: string } | null>(null);
  const [side, setSide] = useState<DrawerSide>(readSide);
  const aside = useRef<HTMLElement>(null);
  const closing = useRef(false);

  const saved = query.data;
  const value = draft ?? saved?.value ?? null;
  const dirty = saved !== undefined && value !== null && JSON.stringify(value) !== JSON.stringify(saved.value);
  const edge: DrawerSide = phone ? "right" : side;
  const look = useMemo(() => (dirty && value && preferences.ready ? appearanceLook(value, preferences.value, group) : null), [dirty, value, preferences.ready, preferences.value, group]);

  useEffect(() => onDraft(look), [look, onDraft]);
  useEffect(() => () => onDraft(null), [onDraft]);
  useEffect(() => aside.current?.focus(), []);

  const finish = useCallback(() => {
    closing.current = true;
    onDraft(null);
    onClose();
  }, [onClose, onDraft]);
  const attemptClose = useCallback(async () => {
    if (saving) return;
    if (dirty) {
      const discard = await confirm({
        title: t("appearance.discardTitle"),
        message: t("appearance.discardBody"),
        confirmLabel: t("appearance.discardConfirm"),
        cancelLabel: t("appearance.keepEditing"),
        destructive: true,
      });
      if (!discard) return;
    }
    finish();
  }, [dirty, finish, saving, t]);

  useBlocker({
    shouldBlockFn: ({ action }) => {
      if (closing.current || action !== "BACK") return false;
      void attemptClose();
      return true;
    },
    enableBeforeUnload: false,
  });

  const save = async () => {
    if (!saved || !value) return;
    const payload = JSON.stringify([value, saved.revision]);
    if (attempt.current?.payload !== payload) attempt.current = { payload, id: crypto.randomUUID() };
    setSaving(true);
    try {
      const next = await saved.save(value, attempt.current.id);
      queryClient.setQueryData(target.key, next);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.groups.all }),
        queryClient.invalidateQueries({ queryKey: queryKeys.characters.all }),
        queryClient.invalidateQueries({ queryKey: queryKeys.conversations.all }),
      ]);
      setDraft(null);
      setError(null);
      attempt.current = null;
    } catch (failure) {
      setError(failure);
    } finally {
      setSaving(false);
    }
  };

  const toggleSide = () => {
    const next = side === "right" ? "left" : "right";
    setSide(next);
    writeSide(next);
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape" && !event.defaultPrevented) {
      event.stopPropagation();
      void attemptClose();
    }
  };

  const offscreen = edge === "right" ? "100%" : "-100%";
  const title = t("settings.views.appearance");
  let body;
  if (!saved || !value) body = query.isError ? <FailureState error={query.error} onRetry={() => void query.refetch()} /> : <LoadingState />;
  else
    body = (
      <div className="space-y-5">
        <fieldset disabled={saving} className="min-w-0">
          <AppearanceEditor draft={value} onChange={setDraft} phone={phone} kind={target.kind} preview={false} />
        </fieldset>
        {error !== null && <FailureState error={error} onRetry={() => void query.refetch({ throwOnError: false }).then(() => setError(null))} />}
      </div>
    );

  return (
    <motion.aside
      ref={aside}
      tabIndex={-1}
      role="dialog"
      aria-label={title}
      onKeyDown={onKeyDown}
      className={cn(
        "fixed inset-y-0 z-50 flex w-[min(30rem,calc(100%-2.5rem))] flex-col overflow-hidden bg-surface-2 pb-[var(--safe-bottom)] pt-[var(--safe-top)] shadow-sheet outline-none",
        edge === "right" ? "right-0 border-l border-line" : "left-0 border-r border-line",
      )}
      initial={{ x: offscreen }}
      animate={{ x: 0, transition: presets.sheetIn }}
      exit={{ x: offscreen, transition: presets.sheetOut }}
    >
      <header className="flex shrink-0 items-center gap-2 border-b border-line px-4 py-3">
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-lg font-semibold text-fg">{title}</h2>
          <p className="truncate text-sm text-fg-3">{group ? t("appearance.forGroup") : t("appearance.forCharacter", { name })}</p>
        </div>
        {!phone && (
          <IconButton label={t(side === "right" ? "appearance.moveLeft" : "appearance.moveRight")} size="sm" shape="round" variant="secondary" onClick={toggleSide}>
            {side === "right" ? <PanelLeftOpen size={icon.md} /> : <PanelRightOpen size={icon.md} />}
          </IconButton>
        )}
        <IconButton label={t("appearance.close")} size="sm" shape="round" variant="secondary" onClick={() => void attemptClose()}>
          <X size={icon.md} />
        </IconButton>
      </header>
      <div className="scrollbar-thin min-h-0 flex-1 overflow-y-auto overscroll-contain px-5 py-4">{body}</div>
      <footer className="flex shrink-0 items-center justify-end gap-2 border-t border-line px-5 py-3">
        <span className="mr-auto text-xs text-fg-3" role="status">{t(dirty ? "appearance.unsaved" : "appearance.saved")}</span>
        {dirty && (
          <>
            <Button variant="ghost" disabled={saving} onClick={() => { setDraft(null); setError(null); attempt.current = null; }}>{t("appearance.discard")}</Button>
            <Button disabled={saving || error !== null} loading={saving} onClick={() => void save()}>{t("appearance.save")}</Button>
          </>
        )}
      </footer>
    </motion.aside>
  );
}
