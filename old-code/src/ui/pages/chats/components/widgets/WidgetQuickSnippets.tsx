import type { QuickSnippetsNode } from "../../../../../core/storage/chatWidgetSchemas";
import { cn, interactive } from "../../../../design-tokens";
import { useI18n } from "../../../../../core/i18n/context";
import { useWidgetContext } from "./WidgetContext";

export function WidgetQuickSnippets({ node }: { node: QuickSnippetsNode }) {
  const { t } = useI18n();
  const { onInsertText } = useWidgetContext();

  return (
    <section className="flex flex-col gap-1.5">
      {(node.title || node.description) && (
        <header className="flex flex-col gap-0.5 px-0.5">
          {node.title && (
            <h3 className="text-sm font-semibold text-fg/75">{node.title}</h3>
          )}
          {node.description && (
            <p className="text-[11px] leading-snug text-fg/45">{node.description}</p>
          )}
        </header>
      )}
      {node.snippets.length === 0 ? (
        <p className="px-0.5 text-[12px] italic text-fg/40">{t("chats.widgets.quickSnippets.empty")}</p>
      ) : (
        <div className="flex flex-wrap gap-1.5">
          {node.snippets.map((snippet) => (
            <button
              key={snippet.id}
              type="button"
              onClick={() => onInsertText(snippet.text)}
              className={cn(
                "rounded-lg border border-fg/15 bg-fg/5 px-2.5 py-1.5 text-[12px] text-fg/80",
                interactive.transition.fast,
                interactive.active.scale,
                "hover:border-fg/30 hover:bg-fg/10",
              )}
              title={snippet.text}
            >
              {snippet.label || snippet.text}
            </button>
          ))}
        </div>
      )}
    </section>
  );
}
