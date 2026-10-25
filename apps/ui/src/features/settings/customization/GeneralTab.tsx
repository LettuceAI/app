import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { CircleCheck, CircleX, Globe, Send, Sparkles, Vibrate } from "lucide-react";
import {
  Button,
  Field,
  icon,
  ListGroup,
  ListRow,
  SelectMenu,
  Slider,
  Switch,
  Text,
} from "@lettuceai/crisp";
import { typingHaptic } from "@/api/haptics";
import { appStatusQuery } from "@/api/app-status-query";
import { matchRequestedLocale } from "@/shared/i18n/device-locale";
import type { SettingsUiAccessibilitySettingsHapticIntensity } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import {
  object,
  usePreferences,
  playSound,
  soundSettings,
  type SoundKey,
} from "@/entities/preferences";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { Row } from "./ChatTab";

const languages = [
  "en",
  "tr",
  "de",
  "fr",
  "es",
  "pt",
  "it",
  "pl",
  "ru",
  "ja",
  "ko",
  "zh-Hans",
  "zh-Hant",
  "id",
  "vi",
  "nl",
  "no",
  "fil",
  "el",
  "hi",
];
const soundIcons = {
  send: <Send size={icon.lg} />,
  success: <CircleCheck size={icon.lg} />,
  failure: <CircleX size={icon.lg} />,
};

export function GeneralTab({ phone }: { phone: boolean }) {
  const { t, i18n } = useTranslation("customization");
  const api = useSettingsApi();
  const client = useQueryClient();
  const status = useQuery(appStatusQuery(api));
  const preferences = usePreferences();
  const [error, setError] = useState<unknown>(null);
  const [languageBusy, setLanguageBusy] = useState(false);
  const accessibility = object(preferences.value.accessibility);
  const mobile = status.data?.platform === "android" || status.data?.platform === "ios";
  const haptics = accessibility.haptics === true;
  const intensity = (accessibility.hapticIntensity ??
    "light") as SettingsUiAccessibilitySettingsHapticIntensity;
  const updateAccessibility = (patch: Record<string, unknown>) =>
    preferences.update((latest) => [
      { key: "accessibility", value: { ...object(latest.accessibility), ...patch } },
    ]);
  const updateSound = (key: SoundKey, patch: { enabled?: boolean; volume?: number }) =>
    preferences.update(
      (latest) => [
        {
          key: "accessibility",
          value: {
            ...object(latest.accessibility),
            [key]: { ...object(object(latest.accessibility)[key]), ...patch },
          },
        },
      ],
      patch.volume !== undefined ? `volume:${key}` : undefined,
    );
  const language =
    status.data?.ui_state.localeChoice === "system"
      ? "system"
      : typeof status.data?.ui_state.locale === "string"
        ? status.data.ui_state.locale
        : i18n.language;
  const setLanguage = async (choice: string) => {
    setLanguageBusy(true);
    setError(null);
    const locale = choice === "system" ? (matchRequestedLocale() ?? "en") : choice;
    try {
      const updated = await api.call("appUiStateUpdate", {
        patch: { locale, localeChoice: choice },
      });
      client.setQueryData(queryKeys.app.status(), (current: typeof status.data) =>
        current ? { ...current, ui_state: updated.state } : current,
      );
      await i18n.changeLanguage(locale);
    } catch (failure) {
      setError(failure);
    } finally {
      setLanguageBusy(false);
    }
  };
  const preview = async (key: SoundKey) => {
    setError(null);
    try {
      await playSound(key, preferences.value, true);
    } catch (failure) {
      setError(failure);
    }
  };
  return (
    <div className="space-y-7">
      {error !== null && <FailureState error={error} size="sm" />}
      <ListGroup label={t("language")}>
        <Row
          phone={phone}
          icon={<Globe size={icon.lg} />}
          title={t("appLanguage")}
          description={t("appLanguageBody")}
          control={
            <Field
              label={<span className="sr-only">{t("appLanguage")}</span>}
              className="space-y-0"
            >
              <SelectMenu
                className={phone ? "w-full" : "w-44"}
                disabled={languageBusy}
                value={language}
                options={[
                  { value: "system", label: t("sameAsSystem") },
                  ...languages.map((value) => ({
                    value,
                    label: new Intl.DisplayNames([value], { type: "language" }).of(value) ?? value,
                  })),
                ]}
                onChange={(value) => {
                  if (value) void setLanguage(value);
                }}
              />
            </Field>
          }
        />
      </ListGroup>
      <section className="space-y-2">
        <ListGroup label={t("soundCues")}>
          {(["send", "success", "failure"] as const).map((key) => {
            const state = soundSettings(preferences.value, key);
            return (
              <div key={key}>
                <ListRow
                  icon={soundIcons[key]}
                  title={t(`sounds.${key}`)}
                  description={t(`sounds.${key}Body`)}
                  trailing={
                    <span className="flex items-center gap-1">
                      <Button size="sm" variant="ghost" onClick={() => void preview(key)}>
                        {t("play")}
                      </Button>
                      <Switch
                        checked={state.enabled}
                        onChange={(enabled) => void updateSound(key, { enabled })}
                        aria-label={t(`sounds.${key}`)}
                      />
                    </span>
                  }
                />
                {state.enabled && (
                  <div className="px-4 pb-3 pl-16">
                    <Slider
                      value={Math.round(state.volume * 100)}
                      onChange={(volume) => void updateSound(key, { volume: volume / 100 })}
                      showValue={(value) => t("percent", { value })}
                      aria-label={t("soundVolume", { sound: t(`sounds.${key}`) })}
                    />
                  </div>
                )}
              </div>
            );
          })}
        </ListGroup>
        <Text variant="hint" className="px-1">
          {t("soundsHint")}
        </Text>
      </section>
      {mobile && (
        <ListGroup label={t("vibration")}>
          <ListRow
            icon={<Vibrate size={icon.lg} />}
            title={t("vibrateWhileReplying")}
            description={t("vibrateBody")}
            trailing={
              <Switch
                checked={haptics}
                onChange={(enabled) => void updateAccessibility({ haptics: enabled })}
                aria-label={t("vibrateWhileReplying")}
              />
            }
          />
          {haptics && (
            <Row
              phone={phone}
              title={t("strength")}
              control={
                <Field
                  label={<span className="sr-only">{t("strength")}</span>}
                  className="space-y-0"
                >
                  <SelectMenu
                    className={phone ? "w-full" : "w-40"}
                    value={intensity}
                    options={(["light", "medium", "heavy", "soft", "rigid"] as const).map(
                      (value) => ({ value, label: t(`haptics.${value}`) }),
                    )}
                    onChange={(value) => {
                      if (value) {
                        void updateAccessibility({ hapticIntensity: value });
                        void typingHaptic(value, status.data?.platform).catch(setError);
                      }
                    }}
                  />
                </Field>
              }
            />
          )}
        </ListGroup>
      )}
      <ListGroup label={t("extras")}>
        <ListRow
          icon={<Sparkles size={icon.lg} />}
          title={t("beetrootRain")}
          description={t("beetrootBody")}
          trailing={
            <Switch
              checked={preferences.value.beetrootRain !== false}
              onChange={(value) => void preferences.update(() => [{ key: "beetrootRain", value }])}
              aria-label={t("beetrootRain")}
            />
          }
        />
      </ListGroup>
    </div>
  );
}
