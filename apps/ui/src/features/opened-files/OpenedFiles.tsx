import { useEffect, useRef, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { BackupHeaderView, FileSource, ImportPreview_Serialize } from "@/api/generated/bindings";
import { createOpenedFilesInbox } from "@/api/opened-files";
import { ImportSheet } from "./ImportSheet";
import { RestoreSheet } from "./RestoreSheet";

interface QueuedFile {
  id: number;
  source: FileSource;
}

type Resolved = { type: "import"; preview: ImportPreview_Serialize } | { type: "restore"; header: BackupHeaderView };

const notRecognized: ReadonlySet<string> = new Set(["unsupported", "malformed", "invalid_input"]);
const importable: ReadonlySet<string> = new Set(["character", "persona", "lorebook"]);

function OpenedFile({ api, source, onDone }: { api: ApiClient; source: FileSource; onDone: () => void }) {
  const { t } = useTranslation("opened-files");
  const { t: common } = useTranslation();
  const [resolved, setResolved] = useState<Resolved | null>(null);
  const done = useRef(onDone);
  useEffect(() => {
    done.current = onDone;
  });

  useEffect(() => {
    let active = true;
    const fail = (error: unknown) => {
      if (!active) return;
      toast.error(notRecognizedError(error) ? t("notRecognized") : common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
      done.current();
    };
    const notRecognizedError = (error: unknown) => isApiFailure(error) && notRecognized.has(error.code);
    void (async () => {
      try {
        const preview = await api.call("importPreview", { source });
        if (importable.has(preview.content.kind)) {
          if (active) setResolved({ type: "import", preview });
          return;
        }
      } catch (error) {
        if (!notRecognizedError(error)) {
          fail(error);
          return;
        }
      }
      try {
        const header = await api.call("backupInspect", { source });
        if (active) setResolved({ type: "restore", header });
      } catch (error) {
        fail(error);
      }
    })();
    return () => { active = false; };
  }, [api, source, t, common]);

  if (!resolved) return null;
  if (resolved.type === "import") return <ImportSheet api={api} source={source} preview={resolved.preview} onClose={onDone} />;
  return <RestoreSheet api={api} source={source} header={resolved.header} onClose={onDone} />;
}

/** Takes the files and navigation requests the shell received and walks the user through them one at a time. */
export function OpenedFiles({ api, held }: { api: ApiClient; held: boolean }) {
  const navigate = useNavigate();
  const [files, setFiles] = useState<QueuedFile[]>([]);
  const counter = useRef(0);
  const holding = useRef(held);
  const waiting = useRef<string[]>([]);
  const go = useRef((path: string) => { void navigate({ to: path as "/" }); });

  useEffect(() => {
    go.current = (path) => { void navigate({ to: path as "/" }); };
  }, [navigate]);

  useEffect(() => {
    holding.current = held;
    if (held) return;
    for (const path of waiting.current.splice(0)) go.current(path);
  }, [held]);

  useEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;
    void createOpenedFilesInbox().then(async (inbox) => {
      if (!inbox || !active) return;
      const pull = async () => {
        const taken = await inbox.take().catch(() => []);
        if (!active) return;
        const added: QueuedFile[] = [];
        for (const item of taken) {
          if (item.kind === "file") added.push({ id: counter.current++, source: item.source });
          else if (holding.current) waiting.current.push(item.path);
          else go.current(item.path);
        }
        if (added.length > 0) setFiles((current) => [...current, ...added]);
      };
      const unlisten = await inbox.listen(() => void pull());
      if (!active) {
        unlisten();
        return;
      }
      stop = unlisten;
      await pull();
    }).catch(() => undefined);
    return () => {
      active = false;
      stop?.();
    };
  }, []);

  const head = files[0];
  if (held || !head) return null;
  return <OpenedFile key={head.id} api={api} source={head.source} onDone={() => setFiles((current) => current.slice(1))} />;
}
