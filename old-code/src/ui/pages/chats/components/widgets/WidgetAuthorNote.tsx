import { useEffect, useRef, useState } from "react";
import type { AuthorNoteNode } from "../../../../../core/storage/chatWidgetSchemas";
import { cn } from "../../../../design-tokens";
import { useI18n } from "../../../../../core/i18n/context";
import { MarkdownRenderer } from "../MarkdownRenderer";
import { useWidgetContext } from "./WidgetContext";
import { useWidgetEdit } from "./WidgetEditContext";
import { widgetCardClass } from "./widgetSurface";

interface WidgetAuthorNoteProps {
  node: AuthorNoteNode;
}

export function WidgetAuthorNote({ node }: WidgetAuthorNoteProps) {
  const { t } = useI18n();
  const { hasBackground, session, onUpdateAuthorNote } = useWidgetContext();
  const { editing: areaEditing } = useWidgetEdit();
  const authorNote = session?.authorNote ?? "";
  const content = authorNote.trim();

  const [inlineEditing, setInlineEditing] = useState(false);
  const [draft, setDraft] = useState(authorNote);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!inlineEditing) setDraft(authorNote);
  }, [authorNote, inlineEditing]);

  useEffect(() => {
    if (inlineEditing) {
      const el = textareaRef.current;
      if (el) {
        el.focus();
        el.setSelectionRange(el.value.length, el.value.length);
      }
    }
  }, [inlineEditing]);

  const canInlineEdit = !areaEditing && !!session;

  const commit = () => {
    setInlineEditing(false);
    if (draft !== authorNote) {
      void onUpdateAuthorNote(draft);
    }
  };

  return (
    <section className="flex flex-col gap-1.5">
      <header className="flex flex-col gap-0.5 px-0.5">
        <h3 className="text-sm font-semibold text-fg/75">{node.title || t("chats.widgets.authorNote.defaultTitle")}</h3>
        {node.description && (
          <p className="text-[11px] leading-snug text-fg/45">{node.description}</p>
        )}
      </header>
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
                setDraft(authorNote);
                setInlineEditing(false);
              }
            }}
            rows={Math.max(3, draft.split("\n").length)}
            className="w-full resize-y bg-transparent text-sm text-fg/85 placeholder-fg/30 focus:outline-none"
            placeholder={t("chats.widgets.authorNote.placeholder")}
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
                  ? t("chats.widgets.authorNote.tapToWrite")
                  : t("chats.widgets.authorNote.empty")}
              </span>
            )}
          </div>
        )}
      </div>
    </section>
  );
}
