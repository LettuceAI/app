import { AnimatePresence, motion } from "framer-motion";
import { useTranslation } from "react-i18next";
import { Avatar, Kbd, motion as m } from "@lettuceai/crisp";
import type { Speaker } from "./SpeakerSheet";

export interface MentionPickerProps {
  matches: readonly Speaker[];
  query: string;
  onPick: (member: Speaker) => void;
}

function Matched({ text, query }: { text: string; query: string }) {
  const at = query ? text.toLowerCase().indexOf(query.toLowerCase()) : -1;
  if (at === -1) return <>{text}</>;
  return (
    <>
      {text.slice(0, at)}
      <span className="font-medium text-accent">{text.slice(at, at + query.length)}</span>
      {text.slice(at + query.length)}
    </>
  );
}

export function MentionPicker({ matches, query, onPick }: MentionPickerProps) {
  const { t } = useTranslation("conversations");
  return (
    <AnimatePresence>
      {matches.length > 0 && (
        <motion.div
          initial={{ opacity: 0, y: 6 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 6 }}
          transition={m.quick}
          className="absolute inset-x-0 bottom-full z-30 mb-2 max-h-48 overflow-y-auto rounded-2xl border border-line bg-surface-el p-1.5 shadow-raised"
          onMouseDown={(event) => event.preventDefault()}
        >
          <p className="px-2 py-1.5 text-xs font-medium text-fg-3">{t("mention.title")}</p>
          <div role="listbox" aria-label={t("mention.title")}>
            {matches.map((member, index) => (
              <button
                key={member.id}
                type="button"
                role="option"
                aria-selected={index === 0}
                onClick={() => onPick(member)}
                className="flex w-full items-center gap-2.5 rounded-xl px-2 py-2 text-left transition-colors hover:bg-fill active:bg-fill-2"
              >
                <Avatar src={member.avatar} name={member.name} size="sm" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm text-fg">
                    <Matched text={member.name} query={query} />
                  </span>
                  {member.description && <span className="block truncate text-xs text-fg-3">{member.description}</span>}
                </span>
                {index === 0 && (
                  <span className="flex shrink-0 items-center gap-1.5 text-2xs text-fg-3">
                    <Kbd>{t("mention.key")}</Kbd>
                    {t("mention.select")}
                  </span>
                )}
              </button>
            ))}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
