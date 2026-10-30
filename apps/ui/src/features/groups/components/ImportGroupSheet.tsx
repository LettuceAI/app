import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Alert, Button, Field, LoadingState, Select, Sheet } from "@lettuceai/crisp";
import { ApiFailure, type ApiClient } from "@/api/client";
import type { FileSource, ImportPreview_Serialize } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { watchJob } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { castQuery } from "../data/queries";
import { refreshGroups } from "../data/actions";

export function ImportGroupSheet({ api, open, onClose, onImported }: { api: ApiClient; open: boolean; onClose: () => void; onImported: () => void }) {
  const { t } = useTranslation("groups");
  const queryClient = useQueryClient();
  const [selected, setSelected] = useState<{ source: FileSource; preview: ImportPreview_Serialize; operationId: string } | null>(null);
  const cast = useQuery(castQuery(api));
  const [speakers, setSpeakers] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const controller = useRef<AbortController | null>(null);
  const activeJob = useRef<string | null>(null);
  useEffect(() => () => controller.current?.abort(), []);
  const pick = async () => {
    if (busy) return;
    setBusy(true); setError(null);
    try {
      const picked = await api.call("filesPickOpen", { kinds: ["chat_log"], multiple: false });
      const source = picked.sources[0];
      if (!source) return;
      const preview = await api.call("importPreview", { source });
      if (preview.content.kind !== "chat" || !preview.content.group) throw new ApiFailure("invalid_input", "Expected a group chat", null);
      activeJob.current = null;
      setSpeakers(Object.fromEntries(preview.content.speakers.map((name) => [name, cast.data?.find((character) => character.name === name)?.id ?? ""])));
      setSelected({ source, preview, operationId: crypto.randomUUID() });
    } catch (error) { setError(error); }
    finally { setBusy(false); }
  };
  const commit = async () => {
    if (!selected || busy || Object.values(speakers).some((id) => !id)) return;
    setBusy(true); setError(null);
    const abort = new AbortController(); controller.current = abort;
    try {
      if (!activeJob.current) {
        const result = await api.call("importCommit", { source: selected.source, digest: selected.preview.digest, options: { speakers }, client_operation_id: selected.operationId });
        if (result.kind === "job") { activeJob.current = result.job_id; setSubmitted(true); }
      }
      if (activeJob.current) {
        const job = await watchJob(api, activeJob.current, () => undefined, abort.signal);
        if (job.state !== "succeeded") { activeJob.current = null; setSubmitted(false); setSelected({ ...selected, operationId: crypto.randomUUID() }); throw new ApiFailure("unavailable", "Group import did not succeed", null); }
      }
      await Promise.all([refreshGroups(queryClient), queryClient.invalidateQueries({ queryKey: queryKeys.characters.all }), queryClient.invalidateQueries({ queryKey: queryKeys.home.all })]);
      setSelected(null); onClose(); onImported();
    } catch (error) { if (!abort.signal.aborted) setError(error); }
    finally { setBusy(false); }
  };
  return <Sheet open={open} onClose={() => { if (!busy) { setSelected(null); setSubmitted(false); activeJob.current = null; setError(null); onClose(); } }} dismissible={!busy} size="sm" title={t("members.import")} footer={<Button variant="primary" block disabled={busy || Boolean(selected && (cast.isPending || cast.isError || Object.values(speakers).some((id) => !id)))} loading={busy} onClick={() => void (selected ? commit() : pick())}>{t(selected ? "import.submit" : "members.chooseFile")}</Button>}>
    <div className="space-y-4 pb-1">{busy && <LoadingState />}{selected ? <Alert title={selected.preview.name} description={t("import.body")} /> : <p className="text-base text-fg-2">{t("members.importBody")}</p>}{selected && selected.preview.content.kind === "chat" && <div className="space-y-3">{cast.isPending ? <LoadingState /> : cast.isError ? <FailureState error={cast.error} onRetry={() => void cast.refetch()} /> : selected.preview.content.speakers.map((name) => <Field key={name} label={name} hint={t("import.mapSpeaker")}><Select disabled={busy || submitted} value={speakers[name] ?? ""} placeholder={t("import.chooseCharacter")} options={(cast.data ?? []).map((character) => ({ value: character.id, label: character.name }))} onChange={(event) => { setSpeakers((current) => ({ ...current, [name]: event.target.value })); setSelected({ ...selected, operationId: crypto.randomUUID() }); }} /></Field>)}</div>}{error !== null && <FailureState error={error} onRetry={() => void (selected ? commit() : pick())} />}</div>
  </Sheet>;
}
