import { useEffect, useMemo, useState, type ComponentProps } from "react";
import { AudioItem } from "@lettuceai/crisp";
import { ApiFailure } from "@/api/client";
import { AudioSource } from "@/api/audio-source";
import { FailureState } from "./FailureState";

type Props = Omit<ComponentProps<typeof AudioItem>, "onLoadSrc" | "onPlaybackError">;

export function AssetAudio(props: Props) {
  return <Player key={props.src ?? ""} {...props} />;
}

function Player({ src, ...props }: Props) {
  const source = useMemo(() => src ? new AudioSource(src) : null, [src]);
  const [error, setError] = useState<unknown>(null);
  useEffect(() => () => source?.release(), [source]);
  const native = src?.startsWith("lettuce-asset:") && source;
  return <div className="min-w-0 space-y-2" onPlayCapture={() => setError(null)}><AudioItem {...props} {...(native ? { onLoadSrc: async () => { setError(null); return native.load(); } } : src === undefined ? {} : { src })} onPlaybackError={(failure) => {
    source?.release();
    setError(failure instanceof ApiFailure ? failure : new ApiFailure("unavailable", "audio playback failed", null));
  }} />{error !== null && <FailureState error={error} size="sm" />}</div>;
}
