import { useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Download, MoreHorizontal, Pencil, Plus, RotateCcw, Trash2, Upload } from "lucide-react";
import {
  Accordion,
  AccordionItem,
  Alert,
  Button,
  cn,
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
  Segmented,
  Text,
  useTheme,
  themeToVars,
  type Theme,
  type ThemeColors,
} from "@lettuceai/crisp";
import type {
  SettingsThemeDocument,
  SettingsUiCustomColorPreset_Deserialize,
  UiPreferenceChange_Deserialize,
} from "@/api/generated/bindings";
import { appStatusQuery } from "@/api/app-status-query";
import { isApiFailure } from "@/api/client";
import {
  appThemes,
  DEFAULT_PRESET_BY_KIND,
  isHexColor,
  kindOfMode,
  rememberedKey,
  themeFromPreferences,
  themeModeOf,
  usePreferences,
  useSystemDark,
  type Preferences,
  type ThemeKind,
  type ThemeMode,
} from "@/entities/preferences";
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

const FEATURED_THEMES = ["midnight", "ink", "paper", "nord", "rose", "oled"] as const;
const ACCENT_SOURCES = [
  "midnight",
  "slate",
  "ember",
  "nord",
  "dracula",
  "rose",
  "plum",
  "carbon-blue",
  "carbon-red",
  "gruvbox",
] as const;
const MORE_THEMES_KEY = "customization.moreThemes";
const featured = FEATURED_THEMES.flatMap((id) => appThemes.filter((theme) => theme.id === id));
const rest = appThemes.filter((theme) => !(FEATURED_THEMES as readonly string[]).includes(theme.id));
const accentSwatches = [
  ...new Set(
    ACCENT_SOURCES.flatMap((id) =>
      appThemes.filter((theme) => theme.id === id).map((theme) => theme.colors.accent.toLowerCase()),
    ),
  ),
];

const readMoreThemes = () => {
  try {
    return window.sessionStorage.getItem(MORE_THEMES_KEY) === "1";
  } catch {
    return false;
  }
};
const writeMoreThemes = (open: boolean) => {
  try {
    window.sessionStorage.setItem(MORE_THEMES_KEY, open ? "1" : "0");
  } catch {
    return;
  }
};

function ThemeTile({
  theme,
  selected,
  onSelect,
  menu,
}: {
  theme: Theme;
  selected: boolean;
  onSelect: () => void;
  menu?: ReactNode;
}) {
  const { surface, fg, accent, nav } = theme.colors;
  return (
    <div className="relative min-w-0">
      <button
        type="button"
        aria-pressed={selected}
        onClick={onSelect}
        className="group flex w-full min-w-0 flex-col gap-1.5 text-left"
      >
        <span
          aria-hidden="true"
          className={cn(
            "flex aspect-[4/3] w-full flex-col overflow-hidden rounded-lg ring-1 ring-inset ring-line-3 transition-shadow group-hover:ring-line-2",
            selected && "ring-2 ring-accent",
          )}
          style={{ background: surface }}
        >
          <span
            className="h-2.5 w-full shrink-0"
            style={nav ? { background: nav } : { background: fg, opacity: 0.1 }}
          />
          <span className="flex flex-1 flex-col justify-center gap-1.5 px-2.5">
            <span className="h-1.5 w-3/4 rounded-full" style={{ background: fg, opacity: 0.85 }} />
            <span className="h-1.5 w-1/2 rounded-full" style={{ background: fg, opacity: 0.4 }} />
            <span className="h-2 w-8 rounded-full" style={{ background: accent }} />
          </span>
        </span>
        <span className="block truncate px-0.5 text-sm text-fg-2">{theme.name}</span>
      </button>
      {menu && <span className="absolute right-1 top-1">{menu}</span>}
    </div>
  );
}

/**
 * Accent precedence: `themeAccent` overrides the accent of whichever theme is active,
 * including a custom one, until it is cleared. Editing the accent under Advanced colors
 * clears `themeAccent` and stores the color as a custom theme, so the two never disagree.
 */
export function ThemeTab({ phone, onRename }: { phone: boolean; onRename: (id: string) => void }) {
  const { t } = useTranslation("customization");
  const api = useSettingsApi();
  const status = useQuery(appStatusQuery(api));
  const preferences = usePreferences();
  const { theme } = useTheme();
  const systemDark = useSystemDark();
  const [error, setError] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const [moreOpen, setMoreOpen] = useState(readMoreThemes);
  const saved = presetsOf(preferences.value.customColorPresets);
  const mode = themeModeOf(preferences.value) ?? (isLight(theme.colors.surface) ? "light" : "dark");
  const appliedChanges = (
    latest: Preferences,
    id: string,
    colors: ThemeColors,
  ): UiPreferenceChange_Deserialize[] => {
    const kind: ThemeKind = isLight(colors.surface) ? "light" : "dark";
    const applied: UiPreferenceChange_Deserialize[] = [
      { key: "themePreset", value: id },
      { key: "theme", value: kind },
      { key: "customColors", value: colors },
    ];
    const current = themeModeOf(latest);
    if (!current) return applied;
    const remembered = { key: rememberedKey(kind), value: id } as UiPreferenceChange_Deserialize;
    const switchMode: UiPreferenceChange_Deserialize[] =
      current !== "system" && current !== kind ? [{ key: "themeMode", value: kind }] : [];
    const visible = current !== "system" || kindOfMode(current, systemDark) === kind;
    return [remembered, ...switchMode, ...(visible ? applied : [])];
  };
  const applyTheme = (next: Theme) =>
    preferences.update((latest) => appliedChanges(latest, next.id, next.colors));
  const changeMode = (next: ThemeMode) =>
    preferences.update((latest) => {
      const kind = next === "system" ? kindOfMode(next, systemDark) : next;
      const shown: UiPreferenceChange_Deserialize[] = [
        { key: "themeMode", value: next },
        { key: "theme", value: kind },
      ];
      if (themeModeOf(latest)) return shown;
      const current = themeFromPreferences({ ...latest, themeAccent: null }, systemDark);
      const currentKind: ThemeKind = isLight(current.colors.surface) ? "light" : "dark";
      const otherKind: ThemeKind = currentKind === "light" ? "dark" : "light";
      return [
        ...shown,
        { key: rememberedKey(currentKind), value: current.id } as UiPreferenceChange_Deserialize,
        {
          key: rememberedKey(otherKind),
          value: DEFAULT_PRESET_BY_KIND[otherKind],
        } as UiPreferenceChange_Deserialize,
      ];
    });
  const setAccent = (accent: string | null) =>
    void preferences.update(() => [{ key: "themeAccent", value: accent }], "themeAccent");
  const handleSave = async (
    name: string,
    colors: ThemeColors,
    shape?: { radius: SettingsThemeDocument["radius"]; density: SettingsThemeDocument["density"] },
  ) => {
    const preset = createThemePreset(name, colors);
    const success = await preferences.update((latest) => [
      { key: "customColorPresets", value: [...presetsOf(latest.customColorPresets), preset] },
      ...appliedChanges(latest, preset.id, colors),
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
      ...(["dark", "light"] as const).flatMap((kind) =>
        latest[rememberedKey(kind)] === preset.id
          ? [{ key: rememberedKey(kind), value: "custom" } as UiPreferenceChange_Deserialize]
          : [],
      ),
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
  const tile = (chosen: Theme, own?: SettingsUiCustomColorPreset_Deserialize) => (
    <ThemeTile
      key={chosen.id}
      theme={chosen}
      selected={theme.id === chosen.id}
      onSelect={() => void applyTheme(chosen)}
      menu={
        own && (
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
        )
      }
    />
  );
  const body = contrastRatio(theme.colors.fg, theme.colors.surface);
  const muted = contrastRatio(
    mixHex(theme.colors.fg, theme.colors.surface, 0.5),
    theme.colors.surface,
  );
  const accent = contrastRatio(theme.colors.accent, theme.colors.surface);
  const grid = phone ? "grid grid-cols-3 gap-3" : "grid grid-cols-6 gap-3";
  const accentNow = theme.colors.accent.toLowerCase();
  const overridden = isHexColor(preferences.value.themeAccent);
  const customAccent = overridden && !accentSwatches.includes(accentNow);
  return (
    <div className="space-y-7">
      {error !== null &&
        (isApiFailure(error) && (error.code === "malformed" || error.code === "invalid_input") ? (
          <Alert tone="danger" title={t("invalidTheme")} description={t("invalidThemeBody")} />
        ) : (
          <FailureState error={error} size="sm" />
        ))}
      <ListGroup>
        <Row
          phone={phone}
          title={t("appearance")}
          description={t("appearanceBody")}
          control={
            <Segmented<ThemeMode>
              size="sm"
              block={phone}
              aria-label={t("appearance")}
              value={mode}
              options={[
                { value: "system", label: t("system") },
                { value: "dark", label: t("dark") },
                { value: "light", label: t("light") },
              ]}
              onChange={(next) => void changeMode(next)}
            />
          }
        />
      </ListGroup>
      <section className="space-y-2">
        <div className="flex items-center justify-between gap-3 px-1">
          <Text variant="label">{t("accent")}</Text>
          {overridden && (
            <Button size="sm" variant="ghost" onClick={() => setAccent(null)}>
              {t("useThemeAccent")}
            </Button>
          )}
        </div>
        <div role="group" aria-label={t("accent")} className="flex flex-wrap items-center gap-3 px-1">
          {accentSwatches.map((hex) => {
            const selected = accentNow === hex;
            return (
              <button
                key={hex}
                type="button"
                aria-pressed={selected}
                aria-label={t("accentColor", { color: hex })}
                onClick={() => setAccent(overridden && selected ? null : hex)}
                className={cn(
                  "h-8 w-8 rounded-full ring-1 ring-inset ring-line-3 transition-shadow",
                  selected && "ring-2 ring-fg ring-offset-2 ring-offset-surface",
                )}
                style={{ background: hex }}
              />
            );
          })}
          <ColorPicker
            value={theme.colors.accent}
            swatches={accentSwatches}
            className={cn(customAccent && "ring-2 ring-fg ring-offset-2 ring-offset-surface")}
            onChange={(color) =>
              void preferences.update(
                () => [{ key: "themeAccent", value: color.toLowerCase() }],
                "themeAccent",
              )
            }
          />
        </div>
        {overridden && accent < 3 && (
          <Alert
            tone="warning"
            title={t("hardToRead")}
            description={t("accentContrast", { ratio: accent.toFixed(1) })}
          />
        )}
      </section>
      <section className="space-y-3">
        <Text variant="label" className="px-1">
          {t("themes")}
        </Text>
        <div className={grid}>
          {(moreOpen ? [...featured, ...rest] : featured)
            .toSorted((a, b) => Number(isLight(a.colors.surface)) - Number(isLight(b.colors.surface)))
            .map((preset) => tile(preset))}
        </div>
        <Button
          size="sm"
          variant="ghost"
          aria-expanded={moreOpen}
          onClick={() => {
            writeMoreThemes(!moreOpen);
            setMoreOpen(!moreOpen);
          }}
        >
          {moreOpen ? t("fewerThemes") : t("moreThemes", { count: rest.length })}
        </Button>
      </section>
      <section className="space-y-3">
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
          <div className={grid}>{saved.map((preset) => tile(asTheme(preset), preset))}</div>
        ) : (
          <Text variant="hint" className="px-1">
            {t("noSavedThemes")}
          </Text>
        )}
      </section>
      <Accordion>
        <AccordionItem
          value="advanced"
          title={t("advancedColors")}
          description={t("advancedColorsBody")}
        >
          <div className="space-y-2">
            <ListGroup>
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
                          theme.colors[key] ??
                          themeToVars(theme)["--color-nav"] ??
                          theme.colors.surface
                        }
                        onChange={(color) =>
                          void preferences.update(
                            (latest) => {
                              const current = themeModeOf(latest);
                              const colors = {
                                ...themeFromPreferences({ ...latest, themeAccent: null }, systemDark)
                                  .colors,
                                [key]: color,
                              };
                              return [
                                { key: "themePreset", value: "custom" },
                                { key: "customColors", value: colors },
                                ...(key === "accent"
                                  ? [{ key: "themeAccent" as const, value: null }]
                                  : []),
                                ...(current
                                  ? [
                                      {
                                        key: rememberedKey(kindOfMode(current, systemDark)),
                                        value: "custom",
                                      } as UiPreferenceChange_Deserialize,
                                    ]
                                  : []),
                              ];
                            },
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
          </div>
        </AccordionItem>
      </Accordion>
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
              { key: "themeMode", value: null },
              { key: "darkThemePreset", value: null },
              { key: "lightThemePreset", value: null },
              { key: "themeAccent", value: null },
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
