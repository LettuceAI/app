import { useEffect, useMemo, useState } from "react";
import { FileCode, Package, Loader2 } from "lucide-react";
import { BottomMenu, MenuButton, MenuButtonGroup, MenuLabel } from "./BottomMenu";
import {
  listCharacterFormats,
  type CharacterFileFormat,
  type CharacterFormatInfo,
} from "../../core/storage/characterTransfer";
import { useI18n } from "../../core/i18n/context";

const FALLBACK_FORMATS: CharacterFormatInfo[] = [
  {
    id: "uec",
    label: "Unified Entity Card (UEC)",
    extension: ".uec",
    canExport: true,
    canImport: true,
    readOnly: false,
  },
  {
    id: "chara_card_v3",
    label: "Character Card V3",
    extension: ".json",
    canExport: true,
    canImport: true,
    readOnly: false,
  },
  {
    id: "chara_card_v2",
    label: "Character Card V2",
    extension: ".json",
    canExport: true,
    canImport: true,
    readOnly: false,
  },
];

const FORMAT_META: Record<
  CharacterFileFormat,
  { icon: typeof Package; color: string; descKey: string }
> = {
  uec: {
    icon: Package,
    color: "from-emerald-500 to-emerald-600",
    descKey: "components.characterExport.formatUecDesc",
  },
  legacy_json: {
    icon: FileCode,
    color: "from-amber-500 to-orange-600",
    descKey: "components.characterExport.formatLegacyJsonDesc",
  },
  chara_card_v3: {
    icon: FileCode,
    color: "from-indigo-500 to-blue-600",
    descKey: "components.characterExport.formatV3Desc",
  },
  chara_card_v2: {
    icon: FileCode,
    color: "from-blue-500 to-cyan-600",
    descKey: "components.characterExport.formatV2Desc",
  },
  chara_card_v1: {
    icon: FileCode,
    color: "from-amber-500 to-amber-600",
    descKey: "components.characterExport.formatV1Desc",
  },
};

interface CharacterExportMenuProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (format: CharacterFileFormat) => void;
  exporting?: boolean;
}

export function CharacterExportMenu({
  isOpen,
  onClose,
  onSelect,
  exporting = false,
}: CharacterExportMenuProps) {
  const { t } = useI18n();
  const [formats, setFormats] = useState<CharacterFormatInfo[]>(FALLBACK_FORMATS);
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    let active = true;
    setLoading(true);
    setLoadError(null);
    listCharacterFormats()
      .then((data) => {
        if (!active) return;
        const fallbackById = new Map(FALLBACK_FORMATS.map((format) => [format.id, format]));
        const merged = new Map<CharacterFileFormat, CharacterFormatInfo>();
        data.forEach((format) => merged.set(format.id, format));
        FALLBACK_FORMATS.forEach((format) => {
          if (!merged.has(format.id)) {
            merged.set(format.id, format);
          }
        });

        const preferredOrder: CharacterFileFormat[] = [
          "uec",
          "chara_card_v3",
          "chara_card_v2",
          "chara_card_v1",
          "legacy_json",
        ];
        const sorted = Array.from(merged.values()).sort((a, b) => {
          const indexA = preferredOrder.indexOf(a.id);
          const indexB = preferredOrder.indexOf(b.id);
          if (indexA === -1 && indexB === -1) return a.label.localeCompare(b.label);
          if (indexA === -1) return 1;
          if (indexB === -1) return -1;
          return indexA - indexB;
        });

        sorted.forEach((format) => {
          if (format.label?.trim()) return;
          const fallback = fallbackById.get(format.id);
          if (fallback) {
            format.label = fallback.label;
            format.extension = fallback.extension;
            format.canExport = fallback.canExport;
            format.canImport = fallback.canImport;
            format.readOnly = fallback.readOnly;
          }
        });

        setFormats(sorted);
      })
      .catch((error: unknown) => {
        if (!active) return;
        setLoadError(error instanceof Error ? error.message : t("components.extra.formatsLoadFailed"));
        setFormats(FALLBACK_FORMATS);
      })
      .finally(() => {
        if (!active) return;
        setLoading(false);
      });

    return () => {
      active = false;
    };
  }, [isOpen, t]);

  const exportableFormats = useMemo(
    () => formats.filter((format) => format.canExport && !format.readOnly),
    [formats],
  );

  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title={t("components.characterExport.title")}>
      <div className="space-y-4">
        <MenuLabel>{t("components.characterExport.selectFormat")}</MenuLabel>
        <MenuButtonGroup>
          {loading && (
            <div className="flex items-center gap-2 rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-white/70">
              <Loader2 className="h-4 w-4 animate-spin" />
              {t("components.characterExport.loading")}
            </div>
          )}
          {!loading &&
            exportableFormats.map((format) => {
              const meta = FORMAT_META[format.id];
              const Icon = meta?.icon ?? FileCode;
              return (
                <MenuButton
                  key={format.id}
                  icon={<Icon className="h-4 w-4" />}
                  title={format.label}
                  description={meta ? t(meta.descKey as any) : t("components.characterExport.formatGeneric", { extension: format.extension })}
                  color={meta?.color ?? "from-blue-500 to-cyan-600"}
                  onClick={() => onSelect(format.id)}
                  disabled={exporting}
                />
              );
            })}
        </MenuButtonGroup>
        {loadError && <p className="text-xs text-amber-200/80">{loadError} {t("components.extra.formatsShowingDefaults")}</p>}
      </div>
    </BottomMenu>
  );
}
