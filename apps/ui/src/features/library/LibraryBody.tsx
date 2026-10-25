import { useTranslation } from "react-i18next";
import { BookOpen, FileAudio, Image, Library, Search, User, UserRound } from "lucide-react";
import {
  Button,
  EmptyState,
  icon,
  MediaCardSkeleton,
  MediaTileSkeleton,
  Skeleton,
} from "@lettuceai/crisp";
import { libraryImagePageSize, type EntryKind, type Filter } from "./data/queries";

export const entryGrid =
  "relative grid grid-cols-2 gap-3 @xl:grid-cols-3 @3xl:grid-cols-4 @5xl:grid-cols-5 @6xl:grid-cols-6";
export const imageGrid =
  "relative grid grid-cols-3 gap-2 @xl:grid-cols-4 @3xl:grid-cols-5 @5xl:grid-cols-7 @6xl:grid-cols-8";
export function LoadingBody({ filter }: { filter: Filter }) {
  if (filter === "images")
    return (
      <div className={imageGrid} aria-busy="true">
        {Array.from({ length: libraryImagePageSize }, (_, i) => (
          <MediaTileSkeleton key={i} />
        ))}
      </div>
    );
  if (filter === "audio")
    return (
      <div className="max-w-3xl space-y-2" aria-busy="true">
        {Array.from({ length: 3 }, (_, i) => (
          <div key={i} className="flex items-center gap-3 rounded-xl border border-line p-3">
            <Skeleton shape="circle" className="h-10 w-10" />
            <div className="flex-1 space-y-2">
              <Skeleton width="45%" className="h-4" />
              <Skeleton width="30%" className="h-3" />
            </div>
          </div>
        ))}
      </div>
    );
  return (
    <div className={entryGrid} aria-busy="true">
      {Array.from({ length: 10 }, (_, i) => (
        <MediaCardSkeleton key={i} />
      ))}
    </div>
  );
}
export function EmptyBody({
  filter,
  query,
  onClear,
  onEverything,
  onCreate,
  onImport,
}: {
  filter: Filter;
  query: string;
  onClear: () => void;
  onEverything: () => void;
  onCreate: (kind: EntryKind) => void;
  onImport: () => void;
}) {
  const { t } = useTranslation("library");
  if (query)
    return (
      <EmptyState
        icon={<Search size={icon.xl} />}
        title={t("empty.matches", { query })}
        description={filter === "all" ? t("empty.searchAll") : t("empty.searchSection")}
        action={
          <div className="flex flex-wrap justify-center gap-2">
            <Button variant="secondary" onClick={onClear}>
              {t("empty.clear")}
            </Button>
            {filter !== "all" && (
              <Button variant="tonal" onClick={onEverything}>
                {t("empty.everything")}
              </Button>
            )}
          </div>
        }
      />
    );
  const icons = {
    all: Library,
    characters: User,
    personas: UserRound,
    lorebooks: BookOpen,
    images: Image,
    audio: FileAudio,
  };
  const Icon = icons[filter];
  const kind =
    filter === "all" || filter === "characters"
      ? "character"
      : filter === "personas"
        ? "persona"
        : filter === "lorebooks"
          ? "lorebook"
          : null;
  return (
    <EmptyState
      icon={<Icon size={icon.xl} />}
      title={t(`empty.${filter}.title`)}
      description={t(`empty.${filter}.description`)}
      action={
        kind && (
          <div className="flex flex-wrap justify-center gap-2">
            <Button variant="tonal" onClick={() => onCreate(kind)}>
              {t(`create.button_${kind}`)}
            </Button>
            {kind !== "persona" && (
              <Button variant="secondary" onClick={onImport}>
                {t("empty.import")}
              </Button>
            )}
          </div>
        )
      }
    />
  );
}
