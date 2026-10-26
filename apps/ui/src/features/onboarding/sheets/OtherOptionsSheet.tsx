import { useState } from "react";
import { ArrowLeft, ArrowRight, FileUp, Languages } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, IconButton, ListGroup, ListRowButton, Sheet } from "@lettuceai/crisp";
import { resources } from "@/shared/i18n";
import { ActionError } from "../components/ActionError";
import { Row, Rows } from "../components/ui";

export interface OtherOptionsSheetProps {
  open: boolean;
  onClose: () => void;
  onSkip: () => void;
  onLanguage: (locale: string) => Promise<void>;
  onRestore: () => void;
  picking: boolean;
  skipping: boolean;
  skipError: unknown;
}

const supportedLocales = Object.keys(resources);

function languageName(locale: string, display: string): string {
  return new Intl.DisplayNames([display], { type: "language" }).of(locale) ?? locale;
}

export function OtherOptionsSheet({ open, onClose, onSkip, onLanguage, onRestore, picking, skipping, skipError }: OtherOptionsSheetProps) {
  const { t, i18n } = useTranslation("onboarding");
  const [view, setView] = useState<"menu" | "language">("menu");
  const [languageError, setLanguageError] = useState<unknown>(null);

  const choose = (locale: string) => {
    setLanguageError(null);
    onLanguage(locale).then(
      () => setView("menu"),
      (error: unknown) => setLanguageError(error),
    );
  };

  return (
    <Sheet
      open={open}
      onClose={onClose}
      title={view === "menu" ? t("other.title") : t("other.languageTitle")}
      leftAction={
        view === "language" ? (
          <IconButton label={t("header.back")} variant="ghost" size="sm" onClick={() => setView("menu")}>
            <ArrowLeft size={icon.md} />
          </IconButton>
        ) : undefined
      }
    >
      {view === "menu" ? (
        <>
          <ListGroup>
            <ListRowButton
              icon={<Languages size={icon.lg} />}
              title={t("other.language")}
              description={languageName(i18n.language, i18n.language)}
              onClick={() => setView("language")}
            />
            <ListRowButton
              icon={<FileUp size={icon.lg} />}
              title={t("other.restore")}
              description={t("other.restoreBody")}
              chevron={false}
              disabled={picking}
              onClick={onRestore}
            />
            <ListRowButton
              icon={<ArrowRight size={icon.lg} />}
              title={t("other.skip")}
              description={t("other.skipBody")}
              chevron={false}
              disabled={skipping}
              onClick={onSkip}
            />
          </ListGroup>
          <div className="mt-3">
            <ActionError error={skipError} busy={skipping} onRetry={onSkip} />
          </div>
        </>
      ) : (
        <>
          <Rows>
            {supportedLocales.map((locale) => (
              <Row
                key={locale}
                selected={i18n.language === locale}
                onClick={() => choose(locale)}
                title={languageName(locale, locale)}
              />
            ))}
          </Rows>
          <div className="mt-3">
            <ActionError error={languageError} busy={false} onRetry={() => setLanguageError(null)} />
          </div>
        </>
      )}
    </Sheet>
  );
}
