import { useCallback, useDeferredValue, useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useBlocker, useCanGoBack, useNavigate, useParams, useRouter } from "@tanstack/react-router";
import { BookOpen, Download, MoreHorizontal, Plus, Radar, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Button,
  confirm,
  EmptyState,
  icon,
  IconButton,
  Menu,
  MenuContent,
  MenuItem,
  MenuTrigger,
  Page,
  ReorderableList,
  Skeleton,
  Text,
  toast,
} from "@lettuceai/crisp";
import { ApiFailure, isApiFailure } from "@/api/client";
import type {
  LorebookEntryView,
  LorebookExportFormat,
  LorebookView_Serialize,
} from "@/api/generated/bindings";
import { useApi } from "@/api/use-api";
import { usePageLayout } from "@/shared/hooks/usePageLayout";
import { FailureState } from "@/shared/ui/FailureState";
import { exportEntry, refreshLibrary } from "../data/actions";
import { lorebookBookKey, lorebookBookQuery, matchesEntry, movedEntry, moveEntry } from "./data";
import { EntryRow } from "./EntryRow";
import { LorebookEntrySheet } from "./LorebookEntrySheet";
import { LorebookExportSheet } from "./LorebookExportSheet";
import { LorebookSettingsSheet } from "./LorebookSettingsSheet";
import { inputOf, useBookWrites } from "./use-book-writes";

function LoadingRows() {
  const { t } = useTranslation("lorebook-editor");
  return (
    <div role="status" aria-label={t("loading")} className="grid gap-3">
      {[0, 1, 2, 3].map((row) => (
        <div key={row} className="space-y-2 rounded-2xl border border-line bg-surface-1 p-4">
          <Skeleton className="h-5 w-40 max-w-full" />
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-2/3" />
        </div>
      ))}
    </div>
  );
}

export function LorebookEditorPage() {
  const { t } = useTranslation("lorebook-editor");
  const { t: library } = useTranslation("library");
  const { t: preview } = useTranslation("lorebook-preview");
  const api = useApi();
  const client = useQueryClient();
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const { phone } = usePageLayout();
  const { lorebookId } = useParams({ from: "/library/lorebooks/$lorebookId" });
  const mainRef = useRef<HTMLElement>(null);
  const book = useQuery(lorebookBookQuery(api, lorebookId));
  const writes = useBookWrites(lorebookId, book.data);
  const entries = writes.entries;
  const [query, setQuery] = useState("");
  const needle = useDeferredValue(query.trim().toLowerCase());
  const [editor, setEditor] = useState<{
    target: LorebookEntryView | "new";
    session: number;
    open: boolean;
  } | null>(null);
  const [settings, setSettings] = useState({ open: false, session: 0 });
  const [exportOpen, setExportOpen] = useState(false);
  const [dragOrder, setDragOrder] = useState<LorebookEntryView[] | null>(null);
  const dragRef = useRef<LorebookEntryView[] | null>(null);
  const pointerDown = useRef(false);
  const dragBase = useRef<string[] | null>(null);
  const entriesRef = useRef(entries);
  const confirmedRef = useRef(book.data?.entries);
  useEffect(() => {
    entriesRef.current = entries;
    confirmedRef.current = book.data?.entries;
  });
  const { mutate } = writes;

  const toLibrary = () =>
    void navigate({ to: "/library", replace: true, state: { libraryKind: "lorebook" } });
  const leave = () => {
    if (canGoBack) router.history.back();
    else toLibrary();
  };
  const filtering = needle.length > 0;
  const visible =
    entries && filtering ? entries.filter((entry) => matchesEntry(entry, needle)) : entries;
  const live = entries?.filter((entry) => entry.enabled).length ?? 0;

  const toggle = useCallback(
    (entry: LorebookEntryView, enabled: boolean) => {
      void mutate(
        { type: "update", entry_id: entry.id, entry: { ...inputOf(entry), enabled } },
        (list) => list.map((item) => (item.id === entry.id ? { ...item, enabled } : item)),
      );
    },
    [mutate],
  );
  const openEditor = useCallback(
    (target: LorebookEntryView | "new") =>
      setEditor((current) => ({ target, session: (current?.session ?? 0) + 1, open: true })),
    [],
  );
  const closeEditor = () => setEditor((current) => current && { ...current, open: false });
  const open = useCallback(
    (entry: LorebookEntryView) =>
      openEditor(confirmedRef.current?.find((item) => item.id === entry.id) ?? entry),
    [openEditor],
  );

  const commitOrder = useCallback(
    (next: LorebookEntryView[]) => {
      const before = dragBase.current ?? entriesRef.current?.map((entry) => entry.id);
      dragBase.current = null;
      dragRef.current = null;
      setDragOrder(null);
      if (!before) return;
      const moved = movedEntry(
        before,
        next.map((entry) => entry.id),
      );
      if (!moved) return;
      void mutate({ type: "reorder", entry_id: moved.id, index: moved.index }, (list) =>
        moveEntry(list, moved.id, moved.index),
      );
    },
    [mutate],
  );
  const reorder = (next: LorebookEntryView[]) => {
    dragRef.current = next;
    setDragOrder(next);
    if (!pointerDown.current) commitOrder(next);
  };
  useEffect(() => {
    const release = () => {
      pointerDown.current = false;
      if (dragRef.current) commitOrder(dragRef.current);
      dragBase.current = null;
    };
    window.addEventListener("pointerup", release);
    window.addEventListener("pointercancel", release);
    return () => {
      window.removeEventListener("pointerup", release);
      window.removeEventListener("pointercancel", release);
    };
  }, [commitOrder]);

  const removeEntry = async (entry: LorebookEntryView) => {
    const confirmed = await confirm({
      title: preview("deleteTitle", { title: entry.title || preview("untitled") }),
      message: preview("deleteBody"),
      confirmLabel: preview("delete"),
      cancelLabel: preview("keep"),
      destructive: true,
    });
    if (!confirmed) return;
    const done = await writes.mutate({ type: "remove", entry_id: entry.id }, (list) =>
      list.filter((item) => item.id !== entry.id),
    );
    if (done) closeEditor();
  };

  const saveName = async (name: string) => {
    const view = await writes.submit((revision) => {
      const current = client.getQueryData<LorebookView_Serialize>(lorebookBookKey(lorebookId));
      if (!current) throw new ApiFailure("internal", "lorebook is not loaded", null);
      return api.call("lorebookUpdateMetadata", {
        lorebook_id: lorebookId,
        expected_revision: revision,
        client_operation_id: crypto.randomUUID(),
        metadata: {
          name,
          detection: current.lorebook.detection,
          icon_asset_id: current.lorebook.icon?.asset_id ?? null,
        },
      });
    });
    writes.noteRevision(view.lorebook.revision);
    client.setQueryData(lorebookBookKey(lorebookId), view);
    void refreshLibrary(client);
  };

  const removal = useMutation({
    mutationFn: async () => {
      const current = book.data?.lorebook;
      if (!current) return false;
      const confirmed = await confirm({
        title: library("delete.title", { name: current.name }),
        message: library("delete.lorebook"),
        confirmLabel: library("delete.confirm"),
        cancelLabel: library("edit.cancel"),
        destructive: true,
      });
      if (!confirmed) return false;
      await writes.submit((revision) =>
        api.call("lorebookDelete", {
          lorebook_id: lorebookId,
          expected_revision: revision,
          client_operation_id: crypto.randomUUID(),
        }),
      );
      toast.success(library("toast.deleted", { name: current.name }));
      await navigate({ to: "/library", replace: true, state: { libraryKind: "lorebook" } });
      void refreshLibrary(client);
      return true;
    },
    onError: writes.fail,
  });

  const exporting = useMutation({
    mutationFn: async (format: LorebookExportFormat) => {
      const current = book.data?.lorebook;
      if (!current) return;
      const result = await exportEntry(
        api,
        { kind: "lorebook", id: lorebookId, name: current.name, description: "", image: null },
        format,
      );
      setExportOpen(false);
      if (!result) return;
      if (result.warnings.length > 0)
        toast.warning(library("toast.warnings", { count: result.warnings.length }));
      else toast.success(library("toast.exported"));
    },
    onError: writes.fail,
  });

  useBlocker({
    shouldBlockFn: ({ action }) => {
      if (action !== "BACK" || removal.isPending || exporting.isPending) return false;
      if (exportOpen) {
        setExportOpen(false);
        return true;
      }
      if (!settings.open) return false;
      setSettings((current) => ({ ...current, open: false }));
      return true;
    },
    enableBeforeUnload: false,
  });

  const missing = isApiFailure(book.error) && book.error.code === "not_found";
  const loaded = book.data;
  const name = loaded?.lorebook.name;
  const total = entries?.length ?? 0;
  const newEntry = () => openEditor("new");
  const sheetEntry = editor?.target === "new" ? null : (editor?.target ?? null);

  const list = (items: LorebookEntryView[]) =>
    filtering ? (
      <div className="grid gap-3">
        {items.map((entry) => (
          <EntryRow key={entry.id} entry={entry} handle={null} onOpen={open} onToggle={toggle} />
        ))}
      </div>
    ) : (
      <div
        onPointerDownCapture={() => {
          pointerDown.current = true;
          dragBase.current = entriesRef.current?.map((entry) => entry.id) ?? null;
        }}
      >
        <ReorderableList
          aria-label={t("entries")}
          items={dragOrder ?? items}
          keyOf={(entry) => entry.id}
          onReorder={reorder}
          className="gap-3"
        >
          {(entry, grip) => (
            <EntryRow
              entry={entry}
              handle={
                <span
                  className={
                    phone
                      ? "mt-0.5"
                      : "mt-0.5 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100"
                  }
                >
                  {grip}
                </span>
              }
              onOpen={open}
              onToggle={toggle}
            />
          )}
        </ReorderableList>
      </div>
    );

  const body = loaded ? (
    visible && visible.length > 0 ? (
      <>
        {list(visible)}
        {!filtering && visible.length > 1 && (
          <Text variant="hint" className="mt-4">
            {t("dragHint")}
          </Text>
        )}
      </>
    ) : (
      <EmptyState
        icon={filtering ? <Search size={icon.xl} /> : <BookOpen size={icon.xl} />}
        title={t(filtering ? "noMatchTitle" : "emptyTitle")}
        description={t(filtering ? "noMatchBody" : "emptyBody")}
        action={
          !filtering && (
            <Button variant="primary" leading={<Plus size={icon.md} />} onClick={newEntry}>
              {t("newEntry")}
            </Button>
          )
        }
        className="pt-16"
      />
    )
  ) : book.isError ? (
    <FailureState error={book.error} {...(missing ? {} : { onRetry: () => void book.refetch() })} />
  ) : (
    <LoadingRows />
  );

  return (
    <div className="relative h-full min-h-0">
      <Page
        className="relative"
        mainRef={mainRef}
        title={name ?? t("title")}
        meta={phone || !loaded ? undefined : t("live", { live, total })}
        onBack={leave}
        backLabel={t("back")}
        {...(loaded
          ? {
              searchValue: query,
              onSearchChange: setQuery,
              searchPlaceholder: t("searchPlaceholder"),
            }
          : {})}
        contentClassName={phone ? "pb-[calc(6rem+var(--safe-bottom,0px))]" : "pb-10"}
        actions={
          loaded && (
            <div className="flex items-center gap-1.5">
              <IconButton
                label={t("triggerPreview")}
                onClick={() =>
                  void navigate({
                    to: "/library/lorebooks/$lorebookId/preview",
                    params: { lorebookId },
                  })
                }
              >
                <Radar size={icon.lg} />
              </IconButton>
              <IconButton
                label={t("settings")}
                onClick={() =>
                  setSettings((current) => ({ open: true, session: current.session + 1 }))
                }
              >
                <BookOpen size={icon.lg} />
              </IconButton>
              {!phone && (
                <IconButton
                  label={t("export")}
                  disabled={exporting.isPending}
                  onClick={() => setExportOpen(true)}
                >
                  <Download size={icon.lg} />
                </IconButton>
              )}
              {!phone && (
                <Button
                  variant="primary"
                  size="sm"
                  leading={<Plus size={icon.sm} />}
                  onClick={newEntry}
                >
                  {t("newEntry")}
                </Button>
              )}
              {phone && (
                <Menu>
                  <MenuTrigger>
                    <IconButton label={t("more")}>
                      <MoreHorizontal size={icon.lg} />
                    </IconButton>
                  </MenuTrigger>
                  <MenuContent align="end">
                    <MenuItem
                      icon={<Download size={icon.md} />}
                      disabled={exporting.isPending}
                      onSelect={() => setExportOpen(true)}
                    >
                      {t("export")}
                    </MenuItem>
                  </MenuContent>
                </Menu>
              )}
            </div>
          )
        }
        footer={
          phone &&
          loaded &&
          visible &&
          visible.length > 0 && (
            <div className="pointer-events-none absolute inset-x-0 bottom-[calc(1.5rem+var(--safe-bottom,0px))] flex justify-end px-6">
              <Button
                className="pointer-events-auto shadow-raised"
                variant="primary"
                leading={<Plus size={icon.md} />}
                onClick={newEntry}
              >
                {t("newEntry")}
              </Button>
            </div>
          )
        }
      >
        {body}
      </Page>
      {loaded && editor && (
        <LorebookEntrySheet
          key={editor.session}
          book={loaded}
          entry={sheetEntry}
          open={editor.open}
          phone={phone}
          onClose={closeEditor}
          {...(sheetEntry ? { onDelete: () => void removeEntry(sheetEntry) } : {})}
          beforeWrite={writes.drain}
          ownRevision={writes.ownRevision}
          onSaved={(view) => writes.noteRevision(view.lorebook.revision)}
        />
      )}
      <LorebookExportSheet
        open={exportOpen}
        busy={exporting.isPending}
        onClose={() => setExportOpen(false)}
        onChoose={(format) => exporting.mutate(format)}
      />
      {loaded && settings.session > 0 && (
        <LorebookSettingsSheet
          key={settings.session}
          open={settings.open}
          name={loaded.lorebook.name}
          deleting={removal.isPending}
          onSave={saveName}
          onClose={() => setSettings((current) => ({ ...current, open: false }))}
          onDelete={() => removal.mutate()}
        />
      )}
    </div>
  );
}
