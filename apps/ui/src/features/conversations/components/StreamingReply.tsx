import { useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { motion } from "framer-motion";
import { Message, motion as m, Progress, Spinner } from "@lettuceai/crisp";
import { spacing, type Author, type Group } from "../data/messages";
import type { GenerationStore } from "../data/generation-store";
import { MessageBody } from "./MessageRow";

export interface StreamingReplyProps {
  store: GenerationStore;
  author: Author | undefined;
  group: Group;
  pinned?: boolean;
}

/** The reply being written. It alone reads the delta text, so a token never re-renders the page. */
export function StreamingReply({ store, author, group, pinned }: StreamingReplyProps) {
  const { t } = useTranslation("conversations");
  const { text, reasoning, loading, imagePrompt } = useSyncExternalStore(store.subscribeText, store.getText);
  const waiting = text.length === 0;
  const state = !waiting ? "streaming" : loading ? "idle" : "typing";
  return (
    <motion.div className={spacing(group)} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} transition={m.quick}>
      <Message
        role="assistant"
        {...(author ? { author } : {})}
        {...(pinned ? { pinned } : {})}
        group={group}
        state={state}
        {...(reasoning ? { reasoning: { text: <MessageBody text={reasoning} query="" /> } } : {})}
        className="mt-0!"
      >
        {waiting && loading ? (
          <div className="min-w-48 space-y-2">
            <p className="text-sm text-fg-2">{t(`loading.${loading.status}`, { model: loading.model, percent: Math.round(loading.percent) })}</p>
            <Progress value={loading.percent} size="sm" label={loading.model} />
          </div>
        ) : (
          <MessageBody text={text} query="" />
        )}
        {imagePrompt && (
          <motion.p
            className="mt-2.5 flex items-center gap-1.5 text-2xs font-medium uppercase tracking-wider text-accent"
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            transition={m.quick}
          >
            <Spinner size="sm" />
            {t("streaming.imagePrompt")}
          </motion.p>
        )}
      </Message>
    </motion.div>
  );
}

export function PendingReply({ author, group }: { author: Author | undefined; group: Group }) {
  return (
    <motion.div className={spacing(group)} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} transition={m.quick}>
      <Message role="assistant" {...(author ? { author } : {})} group={group} state="typing" className="mt-0!" />
    </motion.div>
  );
}
