import { PersonaSheet } from "@/entities/persona";
import { useCallback, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { AnimatePresence, motion } from "framer-motion";
import { ArrowLeft } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, IconButton, motion as presets, Sheet, toast, useCompactLayout } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { ConversationSettingsPatch_Deserialize, ConversationSettingsView, ConversationView } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { pickBackground, updateSettings, type ConversationServices } from "../../data/actions";
import { personasQuery } from "../../data/queries";
import { effectivePersonaId } from "../../data/settings-queries";
import { BackgroundSheet, NoteSheet } from "../ToolSheets";
import { ModelSheet } from "./ModelSheet";
import { LorebooksSheet } from "./LorebooksSheet";
import { PanelViews } from "./PanelViews";
import type { PanelCtx, PanelSheet, PanelView } from "./context";
import { SettingsRoot } from "./SettingsRoot";
import { CompanionTimeSheet } from "./CompanionTimeSheet";

export interface ChatSettingsPanelProps {
  open: boolean;
  api: ApiClient;
  services: ConversationServices;
  conversationId: string;
  view: ConversationView;
  settings: { data: ConversationSettingsView | undefined; isError: boolean; error: unknown; refetch: () => unknown };
  name: string;
  avatar: string | null;
  characterId: string | null;
  onClose: () => void;
  onOpenChat: (conversationId: string, replace: boolean) => void;
  onHome: () => void;
  onMemories: () => void;
  onBranches: () => void;
  onModels: () => void;
}

export function ChatSettingsPanel(props: ChatSettingsPanelProps) {
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const queryClient = useQueryClient();
  const phone = useCompactLayout();
  const { api, services, conversationId, view, settings } = props;
  const [stack, setStack] = useState<PanelView[]>(["root"]);
  const [direction, setDirection] = useState<1 | -1>(1);
  const [busy, setBusy] = useState(false);
  const [sheet, setSheet] = useState<PanelSheet | null>(null);
  const [footerTarget, setFooterTarget] = useState<HTMLDivElement | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const resetViewScroll = useCallback((node: HTMLDivElement | null) => {
    if (node && scrollRef.current) scrollRef.current.scrollTop = 0;
  }, []);
  if (!props.open && (stack.length > 1 || sheet !== null)) {
    setStack(["root"]);
    setSheet(null);
  }
  const current = stack[stack.length - 1] ?? "root";
  const personas = useQuery({ ...personasQuery(api, conversationId), enabled: props.open });

  const run = useCallback(
    async <T,>(task: () => Promise<T>): Promise<T | undefined> => {
      setBusy(true);
      try {
        return await task();
      } catch (error) {
        toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
        return undefined;
      } finally {
        setBusy(false);
      }
    },
    [common],
  );
  const save = useCallback(
    async (change: Partial<ConversationSettingsPatch_Deserialize>) => (await run(() => updateSettings(services, change).then(() => true))) === true,
    [run, services],
  );
  const open = useCallback((next: PanelView) => {
    setDirection(1);
    setStack((previous) => [...previous, next]);
  }, []);
  const openSheet = useCallback((next: PanelSheet) => setSheet(next), []);
  const closeSheet = useCallback(() => setSheet(null), []);
  const back = useCallback(() => {
    setDirection(-1);
    setStack((previous) => (previous.length > 1 ? previous.slice(0, -1) : previous));
  }, []);
  const { onClose, onOpenChat } = props;
  const close = useCallback(() => {
    setStack(["root"]);
    setSheet(null);
    onClose();
  }, [onClose]);

  const ctx: PanelCtx = {
    api, queryClient, services, conversationId, view, settings: settings.data, characterId: props.characterId, busy, activeSheet: sheet, footerTarget, save, run, open, openSheet, closeSheet, back, close, home: props.onHome,
    goTo: (id, replace) => {
      setStack(["root"]);
      onOpenChat(id, replace);
    },
  };

  let body;
  if (settings.isError && !settings.data) {
    body = <FailureState error={settings.error} onRetry={() => void settings.refetch()} />;
  } else if (current === "root") {
    body = <SettingsRoot ctx={ctx} name={props.name} avatar={props.avatar} onOpenMemories={props.onMemories} onBranches={props.onBranches} onModelsSettings={props.onModels} onHome={props.onHome} />;
  } else {
    body = <PanelViews ctx={ctx} current={current} />;
  }

  const personaId = settings.data ? effectivePersonaId(settings.data.persona, personas.data?.defaultId ?? null) : null;

  return (
    <>
    <Sheet
      open={props.open}
      onClose={close}
      dismissible={sheet === null}
      scrollRef={scrollRef}
      footer={current === "advanced" ? <div ref={setFooterTarget} className="@container" /> : undefined}
      title={t(`settings.views.${current}`)}
      size="lg"
      placement="side"
      {...(phone ? { closeButton: false } : {})}
      {...(phone || stack.length > 1
        ? { leftAction: <IconButton label={t("settings.back")} size="sm" shape="round" onClick={stack.length > 1 ? back : close}><ArrowLeft size={icon.lg} /></IconButton> }
        : {})}
    >
      <AnimatePresence mode="wait" initial={false}>
        <motion.div
          key={current}
          ref={resetViewScroll}
          initial={{ opacity: 0, x: direction * 24 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: direction * -24 }}
          transition={presets.quick}
        >
          {body}
        </motion.div>
      </AnimatePresence>
    </Sheet>

      <PersonaSheet
        open={props.open && sheet === "persona"}
        personas={personas.data}
        status={personas.status}
        onRetry={() => void personas.refetch()}
        selectedId={personaId}
        onClose={() => setSheet(null)}
        onSelect={(id) => void save({ persona: id ? { type: "set", id } : { type: "none" } }).then((ok) => ok && setSheet(null))}
      />
      <BackgroundSheet
        open={props.open && sheet === "background"}
        settings={settings.data}
        busy={busy}
        onClose={() => setSheet(null)}
        onPick={() => void run(() => pickBackground(services)).then((done) => done && setSheet(null))}
        onReset={() => void save({ background: { type: "reset" } }).then((ok) => ok && setSheet(null))}
      />
      <NoteSheet
        open={props.open && sheet === "note"}
        saved={settings.data?.author_note ?? null}
        busy={busy}
        onClose={() => setSheet(null)}
        onSave={(note) => void save({ author_note: note }).then((ok) => ok && setSheet(null))}
      />
      <ModelSheet ctx={ctx} open={props.open && sheet === "model"} onClose={() => setSheet(null)} />
      <LorebooksSheet ctx={ctx} open={props.open && sheet === "lorebooks"} onClose={() => setSheet(null)} />
      {props.open && sheet === "companion_time" && settings.data?.companion_time && <CompanionTimeSheet ctx={ctx} value={settings.data.companion_time} />}
    </>
  );
}
