import { useEffect, useRef, useState } from "react";
import type { ScratchPadNode } from "../../../../../core/storage/chatWidgetSchemas";
import { cn } from "../../../../design-tokens";
import { useI18n } from "../../../../../core/i18n/context";
import { MarkdownRenderer } from "../MarkdownRenderer";
import { useWidgetContext } from "./WidgetContext";
import { useWidgetEdit } from "./WidgetEditContext";
import { widgetCardClass } from "./widgetSurface";

interface WidgetScratchPadProps {
  node: ScratchPadNode;
}

export function WidgetScratchPad({ node }: WidgetScratchPadProps) {
  const { t } = useI18n();
  const { hasBackground, onUpdateScratchPad } = useWidgetContext();
  const { editing: areaEditing } = useWidgetEdit();
  const content = node.content?.trim() ?? "";

  const [inlineEditing, setInlineEditing] = useState(false);
  const [draft, setDraft] = useState(node.content ?? "");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!inlineEditing) setDraft(node.content ?? "");
  }, [node.content, inlineEditing]);

  useEffect(() => {
    if (inlineEditing) {
      const el = textareaRef.current;
      if (el) {
        el.focus();
        el.setSelectionRange(el.value.length, el.value.length);
      }
    }
  }, [inlineEditing]);

  const canInlineEdit = !areaEditing;

  const commit = () => {
    setInlineEditing(false);
    if (draft !== (node.content ?? "")) {
      void onUpdateScratchPad(node.id, draft);
    }
  };

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
      <div
        className={cn(
          "rounded-xl px-3 py-2 text-sm text-fg/80",
          widgetCardClass(hasBackground, node.design),
        )}
      >
        {inlineEditing ? (
          <textarea
            ref={textareaRef}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                setDraft(node.content ?? "");
                setInlineEditing(false);
              }
            }}
            rows={Math.max(3, draft.split("\n").length)}
            className="w-full resize-y bg-transparent text-sm text-fg/85 placeholder-fg/30 focus:outline-none"
            placeholder={t("chats.widgets.scratchPad.placeholder")}
          />
        ) : (
          <div
            role={canInlineEdit ? "button" : undefined}
            tabIndex={canInlineEdit ? 0 : undefined}
            onClick={canInlineEdit ? () => setInlineEditing(true) : undefined}
            onKeyDown={
              canInlineEdit
                ? (e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      setInlineEditing(true);
                    }
                  }
                : undefined
            }
            className={canInlineEdit ? "cursor-text" : undefined}
          >
            {content ? (
              <MarkdownRenderer content={content} />
            ) : (
              <span className="text-[12px] italic text-fg/35">
                {canInlineEdit
                  ? t("chats.widgets.scratchPad.tapToWrite")
                  : t("chats.widgets.scratchPad.empty")}
              </span>
            )}
          </div>
        )}
      </div>
    </section>
  );
}
