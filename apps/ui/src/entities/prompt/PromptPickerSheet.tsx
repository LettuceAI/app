import { useState } from "react";
import type { UseQueryResult } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Check, FileText } from "lucide-react";
import {
  CrispStringsProvider,
  EmptyState,
  icon,
  ListGroup,
  ListRowButton,
  LoadingState,
  SearchField,
  Sheet,
} from "@lettuceai/crisp";
import type { PromptKind, PromptSummary } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";

export function PromptPickerSheet({
  open,
  title,
  kind,
  prompts,
  value,
  onChoose,
  onClose,
  onManage,
  defaultChoice,
  manageDescription,
}: {
  open: boolean;
  title: string;
  kind: PromptKind;
  prompts: UseQueryResult<PromptSummary[], Error>;
  value: string | null;
  onChoose: (id: string | null) => void;
  onClose: () => void;
  onManage: () => void;
  defaultChoice?: { title: string; description?: string };
  manageDescription?: string;
}) {
  const { t, i18n } = useTranslation("common");
  const [query, setQuery] = useState("");
  const [previous, setPrevious] = useState({ open, kind });
  if (previous.open !== open || previous.kind !== kind) {
    setPrevious({ open, kind });
    if (open) setQuery("");
  }
  const active = (prompts.data ?? []).filter((prompt) => prompt.kind === kind && !prompt.archived);
  const choices = defaultChoice
    ? active
    : active.filter((prompt) => prompt.origin.type !== "built_in");
  const needle = query.trim().toLocaleLowerCase();
  const found = choices.filter((prompt) => prompt.name.toLocaleLowerCase().includes(needle));
  const defaultSelected =
    value === null ||
    (!defaultChoice &&
      active.some((prompt) => prompt.id === value && prompt.origin.type === "built_in"));
  const mark = <Check size={icon.md} className="text-accent" />;
  return (
    <CrispStringsProvider
      locale={i18n.language}
      strings={{
        sheetClose: t("promptPicker.close"),
        searchFieldClear: t("promptPicker.clearSearch"),
      }}
    >
      <Sheet open={open} onClose={onClose} size="sm" title={title}>
        <div className="space-y-4 pb-1">
          {(choices.length > 8 || query) && (
            <SearchField
              value={query}
              onChange={setQuery}
              placeholder={t("promptPicker.search")}
              aria-label={t("promptPicker.search")}
            />
          )}
          {prompts.isPending ? (
            <LoadingState />
          ) : prompts.isError ? (
            <FailureState error={prompts.error} onRetry={() => void prompts.refetch()} />
          ) : (
            <>
              <ListGroup>
                <ListRowButton
                  icon={<FileText size={icon.lg} />}
                  title={defaultChoice?.title ?? t("promptPicker.builtIn")}
                  description={
                    defaultChoice
                      ? defaultChoice.description
                      : t(
                          active.some((prompt) => prompt.origin.type === "built_in")
                            ? "promptPicker.builtInBody"
                            : "promptPicker.builtInMissing",
                        )
                  }
                  chevron={false}
                  aria-pressed={defaultSelected}
                  trailing={defaultSelected ? mark : undefined}
                  onClick={() => onChoose(null)}
                />
                {found.map((prompt) => (
                  <ListRowButton
                    key={prompt.id}
                    icon={<FileText size={icon.lg} />}
                    title={prompt.name}
                    chevron={false}
                    aria-pressed={prompt.id === value}
                    trailing={prompt.id === value ? mark : undefined}
                    onClick={() => onChoose(prompt.id)}
                  />
                ))}
              </ListGroup>
              {needle && found.length === 0 && (
                <EmptyState size="sm" title={t("promptPicker.noMatches")} />
              )}
            </>
          )}
          <ListGroup>
            <ListRowButton
              icon={<FileText size={icon.lg} />}
              title={t("promptPicker.manage")}
              description={manageDescription ?? t("promptPicker.manageBody")}
              onClick={onManage}
            />
          </ListGroup>
        </div>
      </Sheet>
    </CrispStringsProvider>
  );
}
