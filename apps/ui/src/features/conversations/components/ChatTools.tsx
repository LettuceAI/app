import { PersonaSheet } from "@/entities/persona";
import { useCallback, useEffect, type RefObject } from "react";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { ArrowLeftRight, Dices, FileAudio, Image as ImageIcon, NotebookPen, Sparkles, User } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, ListGroup, ListRowButton, Sheet, toast } from "@lettuceai/crisp";
import type { ApiClient, ApiFailure } from "@/api/client";
import type { ConversationSettingsView, FilePickKind, HelpMeReplyMode } from "@/api/generated/bindings";
import { pickBackground, updateSettings, type ConversationServices } from "../data/actions";
import { helpMeReplyEnabledQuery, modelsQuery, personasQuery } from "../data/queries";
import { effectivePersonaId } from "../data/settings-queries";
import { useHelpReply } from "../data/use-help-reply";
import { attachmentScopes } from "../data/attachment-scopes";
import type { ComposerHandle } from "./ChatComposer";
import { DiceSheet } from "./DiceSheet";
import { HelpChoiceSheet, HelpResultSheet } from "./HelpReplySheets";
import { BackgroundSheet, NoteSheet } from "./ToolSheets";

export type ToolPanel = "menu" | "persona" | "background" | "note" | "dice" | "helpChoice" | "helpResult";

export interface ChatToolsProps {
  api: ApiClient;
  services: ConversationServices;
  conversationId: string;
  settings: ConversationSettingsView | undefined;
  panel: ToolPanel | null;
  setPanel: (panel: ToolPanel | null) => void;
  composer: RefObject<ComposerHandle | null>;
  swap: boolean;
  onSwap: (value: boolean) => void;
  onFailure: (error: unknown) => void;
}

function fileName(uri: string): string {
  const last = uri.split(/[\\/]/).pop() ?? "";
  try {
    return decodeURIComponent(last);
  } catch {
    return last;
  }
}

export function ChatTools({ api, services, conversationId, settings, panel, setPanel, composer, swap, onSwap, onFailure }: ChatToolsProps) {
  const { t } = useTranslation("conversations");
  const [saving, setSaving] = useState(false);
  const helpEnabled = useQuery(helpMeReplyEnabledQuery(api)).data ?? true;
  const models = useQuery(modelsQuery(api));
  const scopes = attachmentScopes(settings, models.data);
  const imageInput = scopes.includes("image");
  const audioInput = scopes.includes("audio");
  const personas = useQuery({ ...personasQuery(api, conversationId), enabled: panel === "menu" || panel === "persona" });

  const helpError = useCallback((failure: ApiFailure) => {
    toast.error(t(`help.callError.${failure.code}`, { defaultValue: t("help.error.failed") }));
    setPanel(null);
  }, [t, setPanel]);
  const help = useHelpReply(api, conversationId, helpError);

  const cancelHelp = help.cancel;
  useEffect(() => {
    if (panel !== "helpResult") cancelHelp();
  }, [panel, cancelHelp]);

  const save = async (task: () => Promise<unknown>, done: () => void) => {
    setSaving(true);
    try {
      await task();
      done();
    } catch (error) {
      onFailure(error);
    } finally {
      setSaving(false);
    }
  };

  const draft = () => composer.current?.read() ?? "";
  const runHelp = (mode: HelpMeReplyMode) => {
    setPanel("helpResult");
    void help.start(mode, draft(), swap);
  };
  const openHelp = () => {
    if (draft().trim()) setPanel("helpChoice");
    else runHelp("new");
  };

  const personaId = settings ? effectivePersonaId(settings.persona, personas.data?.defaultId ?? null) : null;
  const personaHint = personas.isPending
    ? t("persona.loading")
    : personas.isError
      ? t("persona.loadFailed")
      : personaId
        ? personas.data.items.find((persona) => persona.id === personaId)?.title
        : t("persona.none");
  const background = settings?.background;
  const backgroundHint = !background
    ? undefined
    : background.hidden
      ? t("tools.backgroundHidden")
      : background.source === "conversation"
        ? t("tools.backgroundOwn")
        : background.asset
          ? t("tools.backgroundDefault")
          : t("tools.backgroundNone");

  const attach = (kind: Extract<FilePickKind, "image" | "audio">) => {
    setPanel(null);
    void save(async () => {
      const picked = await api.call("filesPickOpen", { kinds: [kind], multiple: true });
      for (const source of picked.sources) {
        const asset = await api.call("assetsIngest", { source, role: "attachment" });
        composer.current?.attach({ id: asset.asset_id, ...(kind === "image" ? { src: asset.url } : {}), name: fileName(source.uri) });
      }
    }, () => undefined);
  };

  const pickImage = () => void save(() => pickBackground(services), () => setPanel(null));

  return (
    <>
      <Sheet open={panel === "menu"} onClose={() => setPanel(null)} size="sm" title={t("tools.title")}>
        <ListGroup className="pb-1">
          {helpEnabled && <ListRowButton icon={<Sparkles size={icon.lg} />} title={t("tools.helpMeReply")} description={t("tools.helpMeReplyHint")} onClick={openHelp} />}
          <ListRowButton icon={<User size={icon.lg} />} title={t("tools.persona")} {...(personaHint ? { description: personaHint } : {})} onClick={() => setPanel("persona")} />
          {imageInput && <ListRowButton icon={<ImageIcon size={icon.lg} />} title={t("tools.uploadImage")} description={t("tools.uploadImageHint")} chevron={false} onClick={() => attach("image")} />}
          {audioInput && <ListRowButton icon={<FileAudio size={icon.lg} />} title={t("tools.uploadAudio")} description={t("tools.uploadAudioHint")} chevron={false} onClick={() => attach("audio")} />}
          <ListRowButton icon={<ImageIcon size={icon.lg} />} title={t("tools.background")} {...(backgroundHint ? { description: backgroundHint } : {})} onClick={() => setPanel("background")} />
          <ListRowButton
            icon={<NotebookPen size={icon.lg} />}
            title={t("tools.note")}
            description={settings?.author_note?.trim() ? t("tools.noteActive") : t("tools.noteInactive")}
            onClick={() => setPanel("note")}
          />
          <ListRowButton icon={<Dices size={icon.lg} />} title={t("tools.dice")} description={t("tools.diceHint")} onClick={() => setPanel("dice")} />
          {settings?.kind !== "group" && (
            <ListRowButton
              icon={<ArrowLeftRight size={icon.lg} />}
              title={swap ? t("tools.swapOn") : t("tools.swap")}
              description={swap ? t("tools.swapOnHint") : t("tools.swapHint")}
              chevron={false}
              onClick={() => {
                onSwap(!swap);
                setPanel(null);
              }}
            />
          )}
        </ListGroup>
      </Sheet>

      <PersonaSheet
        open={panel === "persona"}
        personas={personas.data}
        status={personas.status}
        onRetry={() => void personas.refetch()}
        selectedId={personaId}
        onClose={() => setPanel(null)}
        onSelect={(id) => {
          setPanel(null);
          void save(() => updateSettings(services, { persona: id ? { type: "set", id } : { type: "none" } }), () => undefined);
        }}
      />

      <BackgroundSheet
        open={panel === "background"}
        settings={settings}
        busy={saving}
        onClose={() => setPanel(null)}
        onPick={pickImage}
        onReset={() => void save(() => updateSettings(services, { background: { type: "reset" } }), () => setPanel(null))}
      />

      <NoteSheet
        open={panel === "note"}
        saved={settings?.author_note ?? null}
        busy={saving}
        onClose={() => setPanel(null)}
        onSave={(note) => void save(() => updateSettings(services, { author_note: note }), () => setPanel(null))}
      />

      <DiceSheet
        open={panel === "dice"}
        onClose={() => setPanel(null)}
        onInsert={(text) => {
          composer.current?.append(text);
          setPanel(null);
        }}
      />

      <HelpChoiceSheet open={panel === "helpChoice"} onClose={() => setPanel(null)} onChoose={runHelp} />

      <HelpResultSheet
        open={panel === "helpResult"}
        state={help.state}
        onClose={() => setPanel(null)}
        onRegenerate={() => runHelp(draft().trim() ? "enrich" : "new")}
        onUse={() => {
          composer.current?.replace(help.state.text);
          setPanel(null);
        }}
      />
    </>
  );
}
