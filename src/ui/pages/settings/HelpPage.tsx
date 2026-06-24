import { useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { ArrowLeft, ChevronDown, ChevronRight, HelpCircle, ExternalLink } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { cn, typography } from "../../design-tokens";
import { DISCORD_SERVER_LINK } from "../../../core/utils/links";
import { openExternalUrl } from "../../../core/utils/openExternal";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";

type Translate = (key: TranslationKey, params?: Record<string, string | number>) => string;

interface FaqItem {
  question: TranslationKey;
  answer: (t: Translate) => React.ReactNode;
}

function Crumb() {
  return (
    <ChevronRight
      size={12}
      strokeWidth={2}
      className="mx-0.5 inline-block shrink-0 -translate-y-px text-fg/35"
      aria-hidden
    />
  );
}

interface FaqSection {
  title: TranslationKey;
  items: FaqItem[];
}

const SECTIONS: FaqSection[] = [
  {
    title: "helpPage.sections.gettingStarted",
    items: [
      {
        question: "helpPage.faq.whatIsLettuceai.question",
        answer: (t) => <p>{t("helpPage.faq.whatIsLettuceai.answer")}</p>,
      },
      {
        question: "helpPage.faq.whatIsApiKey.question",
        answer: (t) => (
          <div className="space-y-3">
            <p>{t("helpPage.faq.whatIsApiKey.p1")}</p>
            <p>{t("helpPage.faq.whatIsApiKey.p2")}</p>
            <p>{t("helpPage.faq.whatIsApiKey.p3")}</p>
            <p>{t("helpPage.faq.whatIsApiKey.p4")}</p>
          </div>
        ),
      },
      {
        question: "helpPage.faq.cheapestStart.question",
        answer: (t) => (
          <div className="space-y-2">
            <p>
              <span className="font-medium text-fg">{t("helpPage.faq.cheapestStart.mistralName")}</span>
              {t("helpPage.faq.cheapestStart.mistralBefore")}
              <Crumb />
              {t("helpPage.faq.cheapestStart.mistralAfter")}
            </p>
            <p>
              <span className="font-medium text-fg">{t("helpPage.faq.cheapestStart.cerebrasName")}</span>
              {t("helpPage.faq.cheapestStart.cerebrasText")}
            </p>
            <p>
              <span className="font-medium text-fg">{t("helpPage.faq.cheapestStart.googleName")}</span>
              {t("helpPage.faq.cheapestStart.googleText")}
            </p>
            <p>{t("helpPage.faq.cheapestStart.localText")}</p>
          </div>
        ),
      },
      {
        question: "helpPage.faq.alternative.question",
        answer: (t) => <p>{t("helpPage.faq.alternative.answer")}</p>,
      },
      {
        question: "helpPage.faq.cost.question",
        answer: (t) => <p>{t("helpPage.faq.cost.answer")}</p>,
      },
    ],
  },
  {
    title: "helpPage.sections.privacySafety",
    items: [
      {
        question: "helpPage.faq.apiKeySafe.question",
        answer: (t) => <p>{t("helpPage.faq.apiKeySafe.answer")}</p>,
      },
      {
        question: "helpPage.faq.chatsStored.question",
        answer: (t) => <p>{t("helpPage.faq.chatsStored.answer")}</p>,
      },
      {
        question: "helpPage.faq.chatsDeleted.question",
        answer: (t) => <p>{t("helpPage.faq.chatsDeleted.answer")}</p>,
      },
      {
        question: "helpPage.faq.editRemotely.question",
        answer: (t) => <p>{t("helpPage.faq.editRemotely.answer")}</p>,
      },
      {
        question: "helpPage.faq.shutdown.question",
        answer: (t) => (
          <p>
            {t("helpPage.faq.shutdown.before")}
            <Crumb />
            {t("helpPage.faq.shutdown.after")}
          </p>
        ),
      },
      {
        question: "helpPage.faq.training.question",
        answer: (t) => <p>{t("helpPage.faq.training.answer")}</p>,
      },
    ],
  },
  {
    title: "helpPage.sections.modelsProviders",
    items: [
      {
        question: "helpPage.faq.whatIsModel.question",
        answer: (t) => (
          <div className="space-y-2">
            <p>{t("helpPage.faq.whatIsModel.p1")}</p>
            <p>
              {t("helpPage.faq.whatIsModel.p2Before")}
              <span className="font-medium text-fg">{t("helpPage.faq.whatIsModel.gemmaName")}</span>
              {t("helpPage.faq.whatIsModel.gemmaText")}
              <span className="font-medium text-fg">{t("helpPage.faq.whatIsModel.deepseekName")}</span>
              {t("helpPage.faq.whatIsModel.deepseekText")}
              <span className="font-medium text-fg">{t("helpPage.faq.whatIsModel.glmName")}</span>
              {t("helpPage.faq.whatIsModel.glmText")}
            </p>
          </div>
        ),
      },
      {
        question: "helpPage.faq.freeProviders.question",
        answer: (t) => (
          <p>
            {t("helpPage.faq.freeProviders.before")}
            <span className="font-medium text-fg">{t("helpPage.faq.freeProviders.mistralName")}</span>
            {t("helpPage.faq.freeProviders.afterMistral")}
            <span className="font-medium text-fg">{t("helpPage.faq.freeProviders.cerebrasName")}</span>
            {t("helpPage.faq.freeProviders.afterCerebras")}
            <span className="font-medium text-fg">{t("helpPage.faq.freeProviders.googleName")}</span>
            {t("helpPage.faq.freeProviders.afterGoogle")}
          </p>
        ),
      },
      {
        question: "helpPage.faq.freeVsPaid.question",
        answer: (t) => (
          <div className="space-y-3">
            <p>
              <span className="font-medium text-fg">{t("helpPage.faq.freeVsPaid.freeName")}</span>
              {t("helpPage.faq.freeVsPaid.freeText")}
            </p>
            <p>
              <span className="font-medium text-fg">{t("helpPage.faq.freeVsPaid.paidName")}</span>
              {t("helpPage.faq.freeVsPaid.paidText")}
            </p>
            <p>{t("helpPage.faq.freeVsPaid.mixText")}</p>
          </div>
        ),
      },
      {
        question: "helpPage.faq.whatIsToken.question",
        answer: (t) => (
          <div className="space-y-3">
            <p>{t("helpPage.faq.whatIsToken.p1")}</p>
            <p>{t("helpPage.faq.whatIsToken.p2")}</p>
            <p>{t("helpPage.faq.whatIsToken.p3")}</p>
          </div>
        ),
      },
      {
        question: "helpPage.faq.noDefaultModel.question",
        answer: (t) => (
          <div className="space-y-3">
            <p>{t("helpPage.faq.noDefaultModel.p1")}</p>
            <p>
              {t("helpPage.faq.noDefaultModel.p2Before")}
              <Crumb />
              {t("helpPage.faq.noDefaultModel.p2After")}
            </p>
            <p>{t("helpPage.faq.noDefaultModel.p3")}</p>
          </div>
        ),
      },
      {
        question: "helpPage.faq.cloudVsLocal.question",
        answer: (t) => <p>{t("helpPage.faq.cloudVsLocal.answer")}</p>,
      },
    ],
  },
  {
    title: "helpPage.sections.charactersChats",
    items: [
      {
        question: "helpPage.faq.whatIsCharacter.question",
        answer: (t) => <p>{t("helpPage.faq.whatIsCharacter.answer")}</p>,
      },
      {
        question: "helpPage.faq.whatIsPersona.question",
        answer: (t) => (
          <p>
            {t("helpPage.faq.whatIsPersona.before")}
            <span className="italic">{t("helpPage.faq.whatIsPersona.yourWord")}</span>
            {t("helpPage.faq.whatIsPersona.afterYour")}
            <Crumb />
            {t("helpPage.faq.whatIsPersona.after")}
          </p>
        ),
      },
      {
        question: "helpPage.faq.charactersPrivate.question",
        answer: (t) => <p>{t("helpPage.faq.charactersPrivate.answer")}</p>,
      },
      {
        question: "helpPage.faq.backupMove.question",
        answer: (t) => (
          <p>
            {t("helpPage.faq.backupMove.before")}
            <Crumb />
            {t("helpPage.faq.backupMove.after")}
          </p>
        ),
      },
    ],
  },
] satisfies FaqSection[];

const DOCS_URL = "https://www.lettuceai.app/docs";
const DISCORD_URL = DISCORD_SERVER_LINK;

function openExternal(url: string) {
  void openExternalUrl(url);
}

function FaqRow({
  item,
  isOpen,
  onToggle,
  t,
}: {
  item: FaqItem;
  isOpen: boolean;
  onToggle: () => void;
  t: Translate;
}) {
  return (
    <div className="px-4">
      <button
        type="button"
        onClick={onToggle}
        className="flex w-full items-start gap-4 py-4 text-left"
      >
        <span
          className={cn(
            typography.body.size,
            "flex-1 font-normal leading-snug",
            isOpen ? "text-fg" : "text-fg/85",
          )}
        >
          {t(item.question)}
        </span>
        <motion.span
          animate={{ rotate: isOpen ? 180 : 0 }}
          transition={{ duration: 0.18 }}
          className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center text-fg/35"
        >
          <ChevronDown size={16} strokeWidth={1.75} />
        </motion.span>
      </button>
      <AnimatePresence initial={false}>
        {isOpen && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.18, ease: "easeOut" }}
            className="overflow-hidden"
          >
            <div
              className={cn(
                "pb-5 pr-9 text-[0.9rem] leading-[1.65] text-fg/65 [&_p]:m-0 [&_a]:text-accent [&_a]:underline [&_a]:underline-offset-2",
              )}
            >
              {item.answer(t)}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export function HelpPage() {
  const { t } = useI18n();
  const [openKey, setOpenKey] = useState<string | null>("0-0");
  const location = useLocation();
  const navigate = useNavigate();
  const fromWelcome = Boolean((location.state as { fromWelcome?: boolean } | null)?.fromWelcome);

  return (
    <div className="mx-auto w-full max-w-3xl space-y-8 px-4 py-6 lg:px-8">
      {fromWelcome && (
        <button
          type="button"
          onClick={() => navigate("/welcome")}
          className={cn(
            "inline-flex items-center gap-2 rounded-md border border-fg/10 bg-fg/[0.03] px-3 py-1.5 text-fg/70 transition hover:bg-fg/[0.06] hover:text-fg",
            typography.caption.size,
          )}
        >
          <ArrowLeft size={14} strokeWidth={2} />
          {t("helpPage.backToSetup")}
        </button>
      )}
      <header className="flex items-start gap-3.5">
        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl border border-fg/10 bg-fg/[0.04] text-fg/70">
          <HelpCircle size={20} strokeWidth={1.75} />
        </div>
        <div className="min-w-0">
          <h1 className={cn(typography.h1.size, "font-semibold tracking-tight text-fg")}>
            {t("helpPage.title")}
          </h1>
          <p className={cn("mt-1.5 max-w-prose text-[0.9rem] leading-[1.55] text-fg/55")}>
            {t("helpPage.subtitle")}
          </p>
        </div>
      </header>

      {SECTIONS.map((section, sectionIndex) => (
        <section key={section.title} className="space-y-2">
          <h2
            className={cn(
              "px-1 text-[0.7rem] font-semibold uppercase tracking-[0.14em] text-fg/35",
            )}
          >
            {t(section.title)}
          </h2>
          <div className="overflow-hidden rounded-2xl border border-fg/10 bg-fg/[0.02] divide-y divide-fg/[0.05]">
            {section.items.map((item, itemIndex) => {
              const key = `${sectionIndex}-${itemIndex}`;
              return (
                <FaqRow
                  key={key}
                  item={item}
                  isOpen={openKey === key}
                  onToggle={() => setOpenKey(openKey === key ? null : key)}
                  t={t}
                />
              );
            })}
          </div>
        </section>
      ))}

      <section className="space-y-2">
        <h2
          className={cn(
            typography.overline.size,
            typography.overline.weight,
            typography.overline.tracking,
            typography.overline.transform,
            "px-1 text-fg/40",
          )}
        >
          {t("helpPage.stillStuck.title")}
        </h2>
        <div className="overflow-hidden rounded-xl border border-fg/10 bg-fg/[0.025] divide-y divide-fg/[0.06]">
          <button
            type="button"
            onClick={() => openExternal(DOCS_URL)}
            className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-fg/[0.04]"
          >
            <span className="min-w-0 flex-1">
              <span className={cn("block", typography.body.size, "font-medium text-fg")}>
                {t("helpPage.stillStuck.docsTitle")}
              </span>
              <span className={cn("mt-0.5 block", typography.caption.size, "text-fg/45")}>
                {t("helpPage.stillStuck.docsDescription")}
              </span>
            </span>
            <ExternalLink className="h-4 w-4 shrink-0 text-fg/25" />
          </button>
          <button
            type="button"
            onClick={() => openExternal(DISCORD_URL)}
            className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-fg/[0.04]"
          >
            <span className="min-w-0 flex-1">
              <span className={cn("block", typography.body.size, "font-medium text-fg")}>
                {t("helpPage.stillStuck.discordTitle")}
              </span>
              <span className={cn("mt-0.5 block", typography.caption.size, "text-fg/45")}>
                {t("helpPage.stillStuck.discordDescription")}
              </span>
            </span>
            <ExternalLink className="h-4 w-4 shrink-0 text-fg/25" />
          </button>
        </div>
      </section>
    </div>
  );
}
