import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Download, MoreHorizontal, Pencil, Plus, RotateCcw, Trash2, Upload } from "lucide-react";
import {
  Alert,
  Button,
  ChoiceCard,
  ColorPicker,
  confirm,
  contrastRatio,
  DEFAULT_THEME,
  Field,
  icon,
  IconButton,
  Input,
  isLight,
  ListGroup,
  ListRow,
  Menu,
  MenuContent,
  MenuItem,
  MenuSeparator,
  MenuTrigger,
  mixHex,
  Text,
  useTheme,
  themeToVars,
  type Theme,
  type ThemeColors,
} from "@lettuceai/crisp";
import type {
  SettingsUiCustomColorPreset_Deserialize,
  SettingsThemeDocument,
} from "@/api/generated/bindings";
import { appStatusQuery } from "@/api/app-status-query";
import { isApiFailure } from "@/api/client";
import { appThemes, object, usePreferences } from "@/entities/preferences";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { Pick, Row } from "./ChatTab";
import { createThemePreset } from "./options";

const colorKeys = [
  "surface",
  "fg",
  "accent",
  "nav",
  "danger",
  "warning",
  "info",
  "secondary",
] as const;
const presetsOf = (value: unknown): SettingsUiCustomColorPreset_Deserialize[] =>
  Array.isArray(value) ? (value as SettingsUiCustomColorPreset_Deserialize[]) : [];

function Swatches({ colors }: { colors: ThemeColors }) {
  return (
    <span aria-hidden="true" className="flex -space-x-1">
      {[colors.surface, colors.fg, colors.accent].map((color, index) => (
        <span
          key={index}
          className="h-4 w-4 rounded-full ring-1 ring-line-3"
          style={{ background: color }}
        />
      ))}
    </span>
  );
}

export function ThemeTab({ phone, onRename }: { phone: boolean; onRename: (id: string) => void }) {
  const { t } = useTranslation("customization");
  const api = useSettingsApi();
  const status = useQuery(appStatusQuery(api));
  const preferences = usePreferences();
  const { theme } = useTheme();
  const [error, setError] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const saved = presetsOf(preferences.value.customColorPresets);
  const applyTheme = (next: Theme) =>
    preferences.update(() => [
      { key: "themePreset", value: next.id },
      { key: "theme", value: isLight(next.colors.surface) ? "light" : "dark" },
      { key: "customColors", value: next.colors },
    ]);
  const handleSave = async (
    name: string,
    colors: ThemeColors,
    shape?: { radius: SettingsThemeDocument["radius"]; density: SettingsThemeDocument["density"] },
  ) => {
    const preset = createThemePreset(name, colors);
    const success = await preferences.update((latest) => [
      { key: "customColorPresets", value: [...presetsOf(latest.customColorPresets), preset] },
      { key: "themePreset", value: preset.id },
      { key: "customColors", value: colors },
      { key: "theme", value: isLight(colors.surface) ? "light" : "dark" },
      ...(shape?.radius ? [{ key: "themeRadius" as const, value: shape.radius }] : []),
      ...(shape?.density ? [{ key: "themeDensity" as const, value: shape.density }] : []),
    ]);
    return success;
  };
  const asTheme = (preset: SettingsUiCustomColorPreset_Deserialize): Theme => ({
    id: preset.id ?? "",
    name: preset.name ?? "",
    colors: { ...DEFAULT_THEME.colors, ...preset.colors } as ThemeColors,
  });
  const remove = async (preset: SettingsUiCustomColorPreset_Deserialize) => {
    if (
      !(await confirm({
        title: t("deleteTheme"),
        message: t("deleteThemeBody", { name: preset.name }),
        confirmLabel: t("delete"),
        cancelLabel: t("cancel"),
      }))
    )
      return;
    await preferences.update((latest) => [
      {
        key: "customColorPresets",
        value: presetsOf(latest.customColorPresets).filter((value) => value.id !== preset.id),
      },
      ...(latest.themePreset === preset.id
        ? [{ key: "themePreset" as const, value: "custom" }]
        : []),
    ]);
  };
  const transfer = async (preset?: SettingsUiCustomColorPreset_Deserialize) => {
    setError(null);
    setBusy(true);
    try {
      if (preset) {
        const chosen = asTheme(preset);
        const android = status.data?.platform === "android";
        const target = android
          ? null
          : (
              await api.call("filesPickSave", {
                suggested_name: `${chosen.name.replace(/[^\p{L}\p{N}_. -]/gu, "") || "theme"}.json`,
                kind: "json",
              })
            ).target;
        if (android || target)
          await api.call("settingsThemeExport", {
            document: {
              format_version: 1,
              name: chosen.name,
              colors: { ...chosen.colors },
              radius: theme.radius ?? "default",
              density: theme.density ?? "default",
            },
            target,
          });
      } else {
        const source = (await api.call("filesPickOpen", { kinds: ["json"], multiple: false }))
          .sources[0];
        if (!source) return;
        const imported: SettingsThemeDocument = await api.call("settingsThemeRead", source);
        await handleSave(
          imported.name,
          { ...DEFAULT_THEME.colors, ...imported.colors } as ThemeColors,
          imported,
        );
      }
    } catch (failure) {
      setError(failure);
    } finally {
      setBusy(false);
    }
  };
  const card = (chosen: Theme, own?: SettingsUiCustomColorPreset_Deserialize) => (
    <ChoiceCard
      key={chosen.id}
      selected={theme.id === chosen.id}
      indicator="none"
      title={chosen.name}
      description={
        <span className="flex items-center gap-2">
          <Swatches colors={chosen.colors} />
          {t(isLight(chosen.colors.surface) ? "light" : "dark")}
        </span>
      }
      meta={
        own && (
          <span onClick={(event) => event.stopPropagation()}>
            <Menu>
              <MenuTrigger>
                <IconButton size="sm" label={t("themeActions", { name: chosen.name })}>
                  <MoreHorizontal size={icon.sm} />
                </IconButton>
              </MenuTrigger>
              <MenuContent align="end">
                <MenuItem icon={<Pencil size={icon.md} />} onSelect={() => onRename(chosen.id)}>
                  {t("rename")}
                </MenuItem>
                <MenuItem
                  icon={<Download size={icon.md} />}
                  disabled={busy}
                  onSelect={() => void transfer(own)}
                >
                  {t("export")}
                </MenuItem>
                <MenuSeparator />
                <MenuItem icon={<Trash2 size={icon.md} />} danger onSelect={() => void remove(own)}>
                  {t("delete")}
                </MenuItem>
              </MenuContent>
            </Menu>
          </span>
        )
      }
      onClick={() => void applyTheme(chosen)}
    />
  );
  const body = contrastRatio(theme.colors.fg, theme.colors.surface);
  const muted = contrastRatio(
    mixHex(theme.colors.fg, theme.colors.surface, 0.5),
    theme.colors.surface,
  );
  const accent = contrastRatio(theme.colors.accent, theme.colors.surface);
  const grid = phone ? "grid grid-cols-2 gap-2" : "grid grid-cols-4 gap-2";
  return (
    <div className="space-y-7">
      {error !== null &&
        (isApiFailure(error) && (error.code === "malformed" || error.code === "invalid_input") ? (
          <Alert tone="danger" title={t("invalidTheme")} description={t("invalidThemeBody")} />
        ) : (
          <FailureState error={error} size="sm" />
        ))}
      <section className="space-y-2">
        <Text variant="label" className="px-1">
          {t("themes")}
        </Text>
        <div className={grid}>{appThemes.map((preset) => card(preset))}</div>
      </section>
      <section className="space-y-2">
        <div className="flex items-center justify-between gap-3 px-1">
          <Text variant="label">{t("yourThemes")}</Text>
          <span className="flex gap-1">
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              leading={<Upload size={icon.sm} />}
              onClick={() => void transfer()}
            >
              {t("import")}
            </Button>
            <Button
              size="sm"
              variant="ghost"
              leading={<Plus size={icon.sm} />}
              onClick={() =>
                void handleSave(t("myTheme", { number: saved.length + 1 }), theme.colors)
              }
            >
              {t("saveCurrent")}
            </Button>
          </span>
        </div>
        {saved.length ? (
          <div className={grid}>{saved.map((preset) => card(asTheme(preset), preset))}</div>
        ) : (
          <Text variant="hint" className="px-1">
            {t("noSavedThemes")}
          </Text>
        )}
      </section>
      <section className="space-y-2">
        <ListGroup label={t("colors")}>
          {colorKeys.map((key) => (
            <ListRow
              key={key}
              title={t(`themeColors.${key}`)}
              description={t(`themeColors.${key}Body`)}
              trailing={
                <Field
                  label={<span className="sr-only">{t(`themeColors.${key}`)}</span>}
                  className="space-y-0"
                >
                  <ColorPicker
                    value={
                      theme.colors[key] ?? themeToVars(theme)["--color-nav"] ?? theme.colors.surface
                    }
                    onChange={(color) =>
                      void preferences.update(
                        (latest) => [
                          { key: "themePreset", value: "custom" },
                          {
                            key: "customColors",
                            value: { ...object(latest.customColors), [key]: color },
                          },
                        ],
                        `themeColor:${key}`,
                      )
                    }
                  />
                </Field>
              }
            />
          ))}
        </ListGroup>
        {body < 4.5 || muted < 4.5 || accent < 3 ? (
          <Alert
            tone="warning"
            title={t("hardToRead")}
            description={t(
              body < 4.5 ? "bodyContrast" : muted < 4.5 ? "mutedContrast" : "accentContrast",
              { ratio: (body < 4.5 ? body : muted < 4.5 ? muted : accent).toFixed(1) },
            )}
          />
        ) : (
          <Text variant="hint" className="px-1">
            {t("readableContrast", {
              body: body.toFixed(1),
              muted: muted.toFixed(1),
              accent: accent.toFixed(1),
            })}
          </Text>
        )}
      </section>
      <ListGroup label={t("shape")}>
        <Row
          phone={phone}
          title={t("corners")}
          control={
            <Pick
              label={t("corners")}
              block={phone}
              value={theme.radius ?? "default"}
              options={[
                { value: "sharp", label: t("sharp") },
                { value: "default", label: t("default") },
                { value: "round", label: t("round") },
              ]}
              onChange={(value) => void preferences.update(() => [{ key: "themeRadius", value }])}
            />
          }
        />
        <Row
          phone={phone}
          title={t("density")}
          description={t("densityBody")}
          control={
            <Pick
              label={t("density")}
              block={phone}
              value={theme.density ?? "default"}
              options={[
                { value: "default", label: t("default") },
                { value: "compact", label: t("compact") },
              ]}
              onChange={(value) => void preferences.update(() => [{ key: "themeDensity", value }])}
            />
          }
        />
      </ListGroup>
      <div className="flex justify-end">
        <Button
          variant="ghost"
          size="sm"
          leading={<RotateCcw size={icon.sm} />}
          onClick={() =>
            void preferences.update(() => [
              { key: "themePreset", value: null },
              { key: "theme", value: null },
              { key: "customColors", value: null },
              { key: "themeRadius", value: null },
              { key: "themeDensity", value: null },
            ])
          }
        >
          {t("backToMidnight")}
        </Button>
      </div>
    </div>
  );
}

export function RenameTheme({ id, onDone }: { id: string; onDone: () => void }) {
  const { t } = useTranslation("customization");
  const preferences = usePreferences();
  const preset = presetsOf(preferences.value.customColorPresets).find((preset) => preset.id === id);
  const [name, setName] = useState(preset?.name ?? "");
  const [busy, setBusy] = useState(false);
  const save = async () => {
    if (!name.trim() || busy) return;
    setBusy(true);
    if (
      await preferences.update((latest) => [
        {
          key: "customColorPresets",
          value: presetsOf(latest.customColorPresets).map((preset) =>
            preset.id === id ? { ...preset, name: name.trim() } : preset,
          ),
        },
      ])
    )
      onDone();
    setBusy(false);
  };
  return (
    <div className="space-y-4">
      <Input
        value={name}
        onChange={(event) => setName(event.target.value)}
        aria-label={t("themeName")}
        autoFocus
        onKeyDown={(event) => {
          if (event.key === "Enter") void save();
        }}
      />
      {preferences.error !== null && <FailureState error={preferences.error} size="sm" />}
      <Button block disabled={!preset || !name.trim()} loading={busy} onClick={() => void save()}>
        {t("save")}
      </Button>
    </div>
  );
}
