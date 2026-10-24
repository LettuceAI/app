import { Mic, Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, Progress, Spinner, Text } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import { FailureState } from "@/shared/ui/FailureState";
import { recordingTime } from "./data";
import type { useRecording } from "./use-recording";

export function RecordingFailure({ error }: { error: unknown }) {
  const { t } = useTranslation("settings", { keyPrefix: "speech" });
  const speech = isApiFailure(error) && error.details?.type === "speech" ? error.details.failure.type : null;
  const text = speech === "microphone_permission_denied" ? t("permission") : speech === "no_microphone" ? t("noMicrophone") : speech === "no_audio_captured" ? t("noAudio") : isApiFailure(error) && error.code === "unsupported" ? t("unsupported") : null;
  return text ? <p role="alert" className="text-sm text-danger">{text}</p> : <FailureState error={error} size="sm" />;
}
export function RecordingControls({ recording, modelName, disabled, onStop }: { recording: ReturnType<typeof useRecording>; modelName?: string; disabled?: boolean; onStop: () => void }) {
  const { t } = useTranslation("settings", { keyPrefix: "speech" });
  return <div className="space-y-3">
    {recording.phase === "recording" || recording.phase === "stopping" || recording.phase === "stop_failed" ? <div className="flex items-center gap-3 rounded-xl bg-fill p-3">
      <span className="h-2.5 w-2.5 shrink-0 animate-pulse rounded-full bg-danger" />
      {recording.phase !== "stop_failed" && <Progress value={recording.level} size="sm" tone="danger" label={t("inputLevel")} className="min-w-0 flex-1" />}
      <Text variant="label" className="tabular-nums">{recordingTime(recording.elapsed)}</Text>
      <Button size="sm" variant="tonal" leading={<Square size={icon.sm} />} loading={recording.phase === "stopping"} onClick={onStop}>{t(recording.phase === "stop_failed" ? "retryStop" : "stop")}</Button>{recording.phase === "stop_failed" && <Button size="sm" variant="ghost" onClick={() => void recording.cancel()}>{t("cancel")}</Button>}
    </div> : recording.phase === "transcribing" ? <div className="flex items-center justify-center gap-2 rounded-xl bg-fill p-4 text-sm text-fg-2"><Spinner size="sm" />{t("transcribing", { model: modelName ?? "Whisper" })}</div> : <Button block variant="primary" leading={<Mic size={icon.sm} />} disabled={disabled} loading={recording.phase === "starting"} onClick={() => void recording.start()}>{disabled ? t("recordNeedsModel") : recording.audio ? t("recordAgain") : t("record")}</Button>}
    {recording.error !== null && <RecordingFailure error={recording.error} />}
  </div>;
}
