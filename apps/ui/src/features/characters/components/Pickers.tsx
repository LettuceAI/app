import { useState } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { AudioLines, BookOpen, Check, Cpu, VolumeX } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Checkbox,
  EmptyState,
  icon,
  ListGroup,
  ListRow,
  ListRowButton,
  LoadingState,
  SearchField,
  Sheet,
  Skeleton,
} from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { AudioProviderView, LibraryVoicePreference } from "@/api/generated/bindings";
import { modelsQuery, useChatModels } from "@/features/conversations";
import { ModelMark } from "@/entities/provider";
import { FailureState } from "@/shared/ui/FailureState";
import { voiceLabel } from "../data/voice";
import { audioProvidersQuery, audioVoicesQuery, lorebooksQuery } from "../data/queries";

const mark = <Check size={icon.md} />;

interface ChoiceSheetProps {
  api: ApiClient;
  open: boolean;
  value: string | null;
  onChange: (id: string | null) => void;
  onClose: () => void;
}

/** One model for the character's chats, or the app default. */
export function ModelChoiceSheet({ api, open, value, onChange, onClose }: ChoiceSheetProps) {
  const { t } = useTranslation("characters");
  const status = useQuery(modelsQuery(api));
  const chat = useChatModels(api);
  const [query, setQuery] = useState("");
  const needle = query.trim().toLowerCase();
  const shown = chat.models.filter((model) => !needle || model.name.toLowerCase().includes(needle));
  const choose = (id: string | null) => {
    onChange(id);
    onClose();
  };
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t("pickers.model")}>
      {status.isPending ? (
        <LoadingState />
      ) : status.isError ? (
        <FailureState error={status.error} onRetry={() => void status.refetch()} />
      ) : (
        <div className="space-y-3 pb-1">
          {chat.models.length > 8 && (
            <SearchField
              value={query}
              onChange={setQuery}
              placeholder={t("pickers.modelSearch")}
              aria-label={t("pickers.modelSearch")}
            />
          )}
          <ListGroup>
            <ListRowButton
              icon={<Cpu size={icon.lg} />}
              title={t("details.modelDefault")}
              description={t("pickers.modelDefaultBody")}
              chevron={false}
              trailing={value === null ? mark : undefined}
              onClick={() => choose(null)}
            />
            {shown.map((model) => (
              <ListRowButton
                key={model.id}
                icon={
                  <ModelMark
                    modelId={model.externalId}
                    providerKind={model.providerKind ?? ""}
                    name={model.name}
                  />
                }
                title={model.name}
                description={model.local ? t("pickers.onDevice") : model.provider}
                chevron={false}
                trailing={model.id === value ? mark : undefined}
                onClick={() => choose(model.id)}
              />
            ))}
          </ListGroup>
          {chat.models.length === 0 && (
            <p className="py-2 text-center text-sm text-fg-3">{t("pickers.noModels")}</p>
          )}
        </div>
      )}
    </Sheet>
  );
}

/** Any number of lorebooks, attached in the order they were picked. */
export function LorebookChoiceSheet({
  api,
  open,
  value,
  onChange,
  onClose,
}: {
  api: ApiClient;
  open: boolean;
  value: string[];
  onChange: (ids: string[]) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation("characters");
  const books = useQuery(lorebooksQuery(api));
  const toggle = (id: string, on: boolean) =>
    onChange(on ? [...value, id] : value.filter((entry) => entry !== id));
  return (
    <Sheet
      open={open}
      onClose={onClose}
      size="sm"
      title={t("pickers.lorebooks")}
      footer={
        <Button variant="primary" className="w-full" onClick={onClose}>
          {t("pickers.done")}
        </Button>
      }
    >
      {books.isPending ? (
        <LoadingState />
      ) : books.isError ? (
        <FailureState error={books.error} onRetry={() => void books.refetch()} />
      ) : books.data.length === 0 ? (
        <EmptyState
          size="sm"
          icon={<BookOpen size={icon.xl} />}
          title={t("extras.lorebooksEmpty")}
          description={t("extras.lorebooksEmptyBody")}
        />
      ) : (
        <ListGroup className="mb-1">
          {books.data.map((book) => (
            <ListRow
              key={book.id}
              icon={<BookOpen size={icon.lg} />}
              title={book.name}
              trailing={
                <Checkbox
                  checked={value.includes(book.id)}
                  onChange={(on) => toggle(book.id, on)}
                  aria-label={book.name}
                />
              }
            />
          ))}
        </ListGroup>
      )}
    </Sheet>
  );
}

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
