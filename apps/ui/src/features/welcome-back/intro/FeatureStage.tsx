import { useState, type ReactNode } from "react";
import { motion, type Variants } from "framer-motion";
import { useTranslation } from "react-i18next";
import { cn, ListGroup } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { CreatorKind } from "@/api/generated/bindings";
import { BondTile, EmotionGroup, MemoryRow } from "@/features/conversations";
import { DraftDiff, ReplySteps, Turn } from "@/features/creator";
import { GroupCard, MemberCard, MemoryGroup, type GroupForm } from "@/features/groups";
import { EntryRow } from "@/features/library";
import type { Platform } from "./platform";
import type { Showcase } from "./showcase";

const noop = () => {};
const creatorKind: CreatorKind = "character";

export type FeatureId = "groups" | "companionGroups" | "creator" | "models" | "lorebooks";

const rise: Variants = {
  hidden: { opacity: 0, y: 48, scale: 0.97 },
  show: { opacity: 1, y: 0, scale: 1, transition: { type: "spring", visualDuration: 0.8, bounce: 0.14 } },
  exit: { opacity: 0, y: -28, transition: { duration: 0.3, ease: [0.55, 0, 1, 0.45] } },
};

const featureStage: Variants = {
  hidden: {},
  show: { transition: { staggerChildren: 0.08, delayChildren: 0.25 } },
  exit: { transition: { staggerChildren: 0.03 } },
};

const featureTitle: Variants = {
  hidden: { opacity: 0, y: 16, filter: "blur(8px)" },
  show: { opacity: 1, y: 0, filter: "blur(0px)", transition: { duration: 0.8, ease: [0.22, 1, 0.36, 1] } },
  exit: { opacity: 0, y: -16, filter: "blur(6px)", transition: { duration: 0.3 } },
};

function Rise({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <motion.div variants={rise} className={className}>
      {children}
    </motion.div>
  );
}

interface PieceProps {
  api: ApiClient;
  showcase: Showcase;
  phone: boolean;
}

/** A group as its own screens show it: Home's card or rows, the editor's members with their models, and its memory setting. */
function GroupPieces({ api, showcase, phone }: PieceProps) {
  const { t, i18n } = useTranslation("groups");
  const [form, setForm] = useState<GroupForm>(showcase.groupForm);
  const cards = showcase.groupMembers.slice(0, phone ? 2 : 3).map(({ member, model }, position) => (
    <Rise key={member.id}>
      <MemberCard
        member={member}
        handle={null}
        position={new Intl.NumberFormat(i18n.language).format(position + 1)}
        phone={phone}
        detail={model ?? t("extras.characterModel")}
        onMenu={noop}
      />
    </Rise>
  ));
  if (phone)
    return (
      <div className="w-[330px] space-y-2">
        {showcase.groups.map((entry) => (
          <Rise key={entry.group.id}>
            <GroupCard entry={entry} layout="list" compact held={false} onOpen={noop} onMenu={noop} />
          </Rise>
        ))}
        <div className="space-y-2 pt-2">{cards}</div>
      </div>
    );
  return (
    <div className="grid w-[900px] grid-cols-[210px_1fr_1fr] items-start gap-5">
      <Rise>
        <GroupCard entry={showcase.groups[0]!} layout="gallery" compact={false} held={false} onOpen={noop} onMenu={noop} />
      </Rise>
      <div className="space-y-2">{cards}</div>
      <Rise>
        <MemoryGroup api={api} form={form} update={setForm} dynamicMemory />
      </Rise>
    </div>
  );
}

function Label({ name, role }: { name: string; role: string }) {
  return (
    <p className="mb-2 flex items-baseline gap-2 px-1">
      <span className="text-sm font-semibold text-fg">{name}</span>
      <span className="text-xs text-fg-3">{role}</span>
    </p>
  );
}

/** The pieces each feature page shows, taken from that feature's own screens and data. */
function FeaturePieces({ id, api, showcase, phone }: PieceProps & { id: FeatureId }) {
  const { t } = useTranslation("welcome-back");
  const { companion } = showcase;
  switch (id) {
    case "groups":
      return <GroupPieces api={api} showcase={showcase} phone={phone} />;
    case "companionGroups":
      return (
        <div className={cn("space-y-3", phone ? "w-[330px]" : "w-[640px]")}>
          <div className={cn("grid gap-3", phone ? "grid-cols-2" : "grid-cols-3")}>
            {(["closeness", "trust", "affection"] as const).slice(0, phone ? 2 : 3).map((axis) => (
              <Rise key={axis}>
                <BondTile axis={axis} value={companion.relationship[axis] ?? 0} reference={companion.relationshipDefaults[axis]} narrow={phone} />
              </Rise>
            ))}
          </div>
          <Rise>
            <ListGroup>
              <MemoryRow item={companion.memory} companion onOpen={noop} />
            </ListGroup>
          </Rise>
        </div>
      );
    case "models":
      return (
        <div className={cn("grid items-start gap-6", phone ? "w-[330px] grid-cols-1" : "w-[860px] grid-cols-[1fr_1fr]")}>
          <Rise>
            <div>
              <Label name={t("intro.models.thymos")} role={t("intro.models.thymosRole")} />
              <div className="grid grid-cols-2 gap-2">
                <EmotionGroup group="felt" values={companion.felt} limit={phone ? 3 : 4} />
                <EmotionGroup group="expressed" values={companion.expressed} limit={phone ? 3 : 4} />
              </div>
            </div>
          </Rise>
          <Rise>
            <div>
              <Label name={t("intro.models.eidos")} role={t("intro.models.eidosRole")} />
              <ListGroup>
                {showcase.chatMemories.slice(0, phone ? 1 : 2).map((memory) => (
                  <MemoryRow key={memory.id} item={memory} onOpen={noop} />
                ))}
              </ListGroup>
            </div>
          </Rise>
        </div>
      );
    case "creator": {
      const { turn, before, after } = showcase.creator;
      const reply = <ReplySteps api={api} turn={turn} canAct onUseAvatar={noop} onEditImage={noop} />;
      const conversation = <Turn api={api} turn={turn} canAct steps={reply} live={null} onChanges={noop} onRegenerate={noop} />;
      return phone ? (
        <div className="w-[400px] space-y-3 [zoom:0.78]">
          <Rise>{conversation}</Rise>
        </div>
      ) : (
        <div className="grid w-[960px] grid-cols-[1fr_380px] items-start gap-6 [zoom:0.86]">
          <Rise>
            <div className="space-y-3">{conversation}</div>
          </Rise>
          <Rise>
            <div className="rounded-2xl border border-line bg-surface-1 p-4">
              <p className="mb-4 text-sm font-semibold text-fg">{t("intro.creator.review")}</p>
              <DraftDiff before={before} after={after} kind={creatorKind} />
            </div>
          </Rise>
        </div>
      );
    }
    case "lorebooks":
      return (
        <div className={cn("space-y-3", phone ? "w-[330px]" : "w-[560px]")}>
          {showcase.lorebook.slice(0, phone ? 2 : 3).map((entry) => (
            <Rise key={entry.id}>
              <EntryRow entry={entry} handle={null} onOpen={noop} onToggle={noop} />
            </Rise>
          ))}
        </div>
      );
  }
}

/** A feature as the redesign page is: its name large in the middle, its real pieces rising in beneath. */
export function FeatureStage({ id, platform, api, showcase }: { id: FeatureId; platform: Platform; api: ApiClient; showcase: Showcase }) {
  const { t } = useTranslation("welcome-back");
  const phone = platform === "phone";
  return (
    <motion.div
      variants={featureStage}
      initial="hidden"
      animate="show"
      exit="exit"
      className={cn("absolute inset-0 flex flex-col items-center overflow-hidden text-center", phone ? "justify-start gap-6 px-6 pb-44 pt-10" : "justify-center-safe gap-10 px-12 pb-28 pt-8")}
    >
      <motion.div variants={featureTitle} className="space-y-4">
        <h1 className={cn("text-balance font-bold tracking-[-0.04em] text-fg", phone ? "text-[2.3rem] leading-[1.05]" : "text-6xl leading-[1.02]")}>{t(`intro.${id}.title`)}</h1>
        <p className={cn("mx-auto max-w-xl text-balance text-fg-2", phone ? "text-base" : "text-xl")}>{t(`intro.${id}.body`)}</p>
      </motion.div>
      <div className="text-left">
        <FeaturePieces id={id} api={api} showcase={showcase} phone={phone} />
      </div>
    </motion.div>
  );
}
