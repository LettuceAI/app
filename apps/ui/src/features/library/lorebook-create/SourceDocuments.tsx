import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Dropzone, fileSize, Text } from "@lettuceai/crisp";
import { listenFileDrops } from "@/api/file-drops";
import { ApiFailure } from "@/api/client";
import type { FileSource } from "@/api/generated/bindings";
import { useApi } from "@/api/use-api";
import { ActionError } from "@/shared/ui/ActionError";
import { fileLimit, totalSourceLimit, type DocumentSource } from "../data/lorebook-projects";

export function SourceDocuments({
  bytes,
  disabled,
  onAdd,
  onBusy,
  showSize,
}: {
  bytes: number;
  disabled: boolean;
  onAdd: (documents: DocumentSource[]) => void;
  onBusy: (busy: boolean) => void;
  showSize: boolean;
}) {
  const { t, i18n } = useTranslation("lorebook-create");
  const api = useApi();
  const area = useRef<HTMLDivElement>(null);
  const alive = useRef(true);
  const busyRef = useRef(false);
  const pickerBusy = useRef(false);
  const bytesValue = useRef(bytes);
  const [watchAttempt, setWatchAttempt] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [issue, setIssue] = useState<string | null>(null);
  const retry = useRef<() => void>(() => {});
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      onBusy(false);
    };
  }, [onBusy]);
  useEffect(() => {
    bytesValue.current = bytes;
  }, [bytes]);
  const add = useCallback(
    async function addSources(sources: FileSource[]): Promise<void> {
      if (busyRef.current || disabled) return;
      busyRef.current = true;
      setBusy(true);
      onBusy(true);
      setError(null);
      setIssue(null);
      let remaining = sources;
      retry.current = () => void addSources(remaining);
      try {
        let total = bytesValue.current;
        for (let index = 0; index < sources.length; index++) {
          remaining = sources.slice(index);
          const source = sources[index]!;
          const inspected = await api.call("filesInspect", { source });
          if (!alive.current) return;
          if (!/\.(txt|text|md|markdown|pdf)$/i.test(inspected.name)) {
            setIssue(t("unsupportedFile", { name: inspected.name }));
            continue;
          }
          if (inspected.size > fileLimit) {
            setIssue(t("fileTooLarge", { name: inspected.name }));
            continue;
          }
          if (total + inspected.size > totalSourceLimit) {
            setIssue(t("sourcesTooLarge"));
            break;
          }
          const asset = await api.call("assetsIngest", { source, role: "source_document" });
          if (!alive.current) return;
          total += inspected.size;
          bytesValue.current = total;
          onAdd([{ id: asset.asset_id, label: inspected.name, size: inspected.size }]);
          remaining = sources.slice(index + 1);
        }
      } catch (failure) {
        if (alive.current) setError(failure);
      } finally {
        busyRef.current = false;
        if (alive.current) {
          setBusy(false);
          onBusy(false);
        }
      }
    },
    [api, disabled, onAdd, onBusy, t],
  );
  const pick = async () => {
    if (busyRef.current || pickerBusy.current || disabled) return;
    pickerBusy.current = true;
    setBusy(true);
    onBusy(true);
    setError(null);
    try {
      const picked = await api.call("filesPickOpen", { kinds: ["any"], multiple: true });
      if (alive.current) await add(picked.sources);
    } catch (failure) {
      if (alive.current) {
        setError(failure);
        retry.current = () => void pick();
      }
    } finally {
      pickerBusy.current = false;
      if (alive.current) {
        setBusy(false);
        onBusy(false);
      }
    }
  };
  useEffect(() => {
    if (api.transport !== "tauri") return;
    let stop: (() => void) | undefined;
    let disposed = false;
    void listenFileDrops((paths, position) => {
      if (!area.current || disabled) return;
      const bounds = area.current.getBoundingClientRect();
      const x = position.x / window.devicePixelRatio;
      const y = position.y / window.devicePixelRatio;
      if (x >= bounds.left && x <= bounds.right && y >= bounds.top && y <= bounds.bottom)
        void add(paths.map((uri) => ({ uri })));
    })
      .then((unsubscribe) => {
        if (disposed) unsubscribe();
        else stop = unsubscribe;
      })
      .catch((failure: unknown) => {
        if (!disposed) {
          setError(failure);
          retry.current = () => {
            setError(null);
            setWatchAttempt((value) => value + 1);
          };
        }
      });
    return () => {
      disposed = true;
      stop?.();
    };
  }, [api, add, disabled, watchAttempt]);
  return (
    <div className="space-y-2">
      <div
        ref={area}
        onClickCapture={(event) => {
          event.preventDefault();
          event.stopPropagation();
          void pick();
        }}
        onKeyDownCapture={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            event.stopPropagation();
            void pick();
          }
        }}
      >
        <Dropzone
          multiple
          accept=".txt,.text,.md,.markdown,.pdf"
          maxSize={fileLimit}
          onReject={(files, reason) =>
            setIssue(
              t(reason === "size" ? "fileTooLarge" : "unsupportedFile", {
                name: files[0]?.name ?? "",
              }),
            )
          }
          disabled={disabled || busy}
          onFiles={(files) => {
            const sources = files.flatMap((file) =>
              typeof (file as File & { path?: string }).path === "string"
                ? [{ uri: (file as File & { path: string }).path }]
                : [],
            );
            if (sources.length === files.length) void add(sources);
            else setError(new ApiFailure("unsupported", "source file paths unavailable", null));
          }}
          hint={t(busy ? "addingFiles" : "filesHint")}
        />
      </div>
      {bytes > 0 && showSize && (
        <Text variant="hint" className="px-1">
          {t("sourceSize", { value: fileSize(bytes, i18n.language) })}
        </Text>
      )}
      {issue && (
        <Text role="alert" variant="hint">
          {issue}
        </Text>
      )}
      <ActionError error={error} busy={busy} onRetry={() => retry.current()} />
    </div>
  );
}
