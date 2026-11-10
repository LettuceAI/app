import { useQueries, useQuery } from "@tanstack/react-query";
import { AudioLines, Check, VolumeX } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  icon,
  ListGroup,
  ListRow,
  ListRowButton,
  LoadingState,
  Sheet,
  Skeleton,
} from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { AudioProviderView, LibraryVoicePreference } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { voiceLabel } from "../data/voice";
import { audioProvidersQuery, audioVoicesQuery } from "../data/queries";

const mark = <Check size={icon.md} />;

function VoiceGroup({
  api,
  provider,
  value,
  onChoose,
}: {
  api: ApiClient;
  provider: AudioProviderView;
  value: LibraryVoicePreference | null;
  onChoose: (voice: LibraryVoicePreference) => void;
}) {
  const { t } = useTranslation("characters");
  const { t: common } = useTranslation("common");
  const voices = useQuery(audioVoicesQuery(api, provider.id));
  return (
    <ListGroup label={provider.label}>
      {voices.isPending ? (
        <ListRow title={<Skeleton className="h-4 w-32" />} />
      ) : voices.isError ? (
        <ListRowButton
          title={common(
            `errors.code.${isApiFailure(voices.error) ? voices.error.code : "internal"}`,
          )}
          chevron={false}
          trailing={<span className="text-sm text-fg-3">{common("actions.retry")}</span>}
          onClick={() => void voices.refetch()}
        />
      ) : voices.data.length === 0 ? (
        <ListRow title={t("voice.noVoices")} />
      ) : (
        voices.data.map((voice) => (
          <ListRowButton
            key={voice.voice_id}
            icon={<AudioLines size={icon.lg} />}
            title={voice.name}
            description={Object.values(voice.labels).join(", ") || undefined}
            chevron={false}
            trailing={
              value?.kind === "provider" &&
              value.value.provider_id === provider.id &&
              value.value.voice_id === voice.voice_id
                ? mark
                : undefined
            }
            onClick={() =>
              onChoose({
                kind: "provider",
                value: {
                  provider_id: provider.id,
                  voice_id: voice.voice_id,
                  model_id: null,
                  voice_name: voice.name,
                },
              })
            }
          />
        ))
      )}
    </ListGroup>
  );
}

export function VoiceChoiceSheet({
  api,
  open,
  value,
  onChange,
  onClose,
}: {
  api: ApiClient;
  open: boolean;
  value: LibraryVoicePreference | null;
  onChange: (voice: LibraryVoicePreference | null) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation("characters");
  const providers = useQuery(audioProvidersQuery(api));
  const listings = useQueries({
    queries: (providers.data ?? []).map((provider) => audioVoicesQuery(api, provider.id)),
  });
  const unlisted =
    value?.kind === "provider" &&
    listings.every((listing) => !listing.isPending) &&
    !(providers.data ?? []).some(
      (provider, index) =>
        provider.id === value.value.provider_id &&
        listings[index]?.data?.some((voice) => voice.voice_id === value.value.voice_id),
    );
  const choose = (voice: LibraryVoicePreference | null) => {
    onChange(voice);
    onClose();
  };
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t("voice.title")}>
      {providers.isPending ? (
        <LoadingState />
      ) : providers.isError ? (
        <FailureState error={providers.error} onRetry={() => void providers.refetch()} />
      ) : (
        <div className="space-y-3 pb-1">
          <ListGroup>
            <ListRowButton
              icon={<VolumeX size={icon.lg} />}
              title={t("voice.none")}
              description={t("voice.noneBody")}
              chevron={false}
              trailing={value === null ? mark : undefined}
              onClick={() => choose(null)}
            />
            {value && (value.kind !== "provider" || unlisted) && (
              <ListRowButton
                icon={<AudioLines size={icon.lg} />}
                title={voiceLabel(value, t("voice.saved"))}
                chevron={false}
                trailing={mark}
                onClick={onClose}
              />
            )}
          </ListGroup>
          {providers.data.length === 0 && (
            <p className="py-2 text-center text-sm text-fg-3">{t("voice.noProviders")}</p>
          )}
          {providers.data.map((provider) => (
            <VoiceGroup
              key={provider.id}
              api={api}
              provider={provider}
              value={value}
              onChoose={choose}
            />
          ))}
        </div>
      )}
    </Sheet>
  );
}
