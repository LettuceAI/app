import { useSyncExternalStore } from "react";
import { motion } from "framer-motion";
import { Message, motion as m } from "@lettuceai/crisp";
import { spacing, type Author, type Group } from "../data/messages";
import type { GenerationStore } from "../data/generation-store";
import { MessageBody } from "./MessageRow";

export interface StreamingReplyProps {
  store: GenerationStore;
  author: Author | undefined;
  group: Group;
  pinned?: boolean;
}

export function StreamingReply({ store, author, group, pinned }: StreamingReplyProps) {
  const { text, reasoning, loading, imagePrompt } = useSyncExternalStore(store.subscribeText, store.getText);
  const waiting = text.length === 0;
  const preparing = loading !== null || imagePrompt;
  const state = preparing ? "idle" : waiting ? "typing" : "streaming";
  if (preparing && waiting && !reasoning) return null;
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
        <MessageBody text={text} query="" />
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
