import { useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useCanGoBack, useNavigate, useParams, useRouteContext, useRouter } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, LoadingState, Page, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import { FailureState } from "@/shared/ui/FailureState";
import { startDirectChat } from "../data/actions";
import { characterQuery } from "../data/settings-queries";
import { ChatHistoryList } from "./ChatHistoryList";

export function ChatHistoryPage() {
  const { characterId } = useParams({ from: "/characters/$characterId/history" });
  return <HistoryScreen key={characterId} characterId={characterId} />;
}

function HistoryScreen({ characterId }: { characterId: string }) {
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const { api, queryClient } = useRouteContext({ from: "__root__" });
  const character = useQuery(characterQuery(api, characterId));
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const [actionsOpen, setActionsOpen] = useState(false);
  const run = async <T,>(task: () => Promise<T>): Promise<T | undefined> => {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    try {
      return await task();
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      running.current = false;
      setBusy(false);
    }
  };
  const openChat = (conversationId: string) => {
    void navigate({ to: "/chat/$conversationId", params: { conversationId } });
  };
  const start = async () => {
    const id = await run(() => startDirectChat(api, queryClient, characterId));
    if (id) openChat(id);
  };
  const newChat = (
    <Button variant="primary" size="sm" leading={<Plus size={icon.sm} />} disabled={busy || !character.isSuccess || character.data.archived} onClick={() => void start()}>
      {t("settings.views.starters")}
    </Button>
  );

  return (
    <Page
      title={t("settings.views.history")}
      backLabel={t("header.back")}
      onBack={() => canGoBack ? router.history.back() : void navigate({ to: "/" })}
      actions={newChat}
    >
      {character.isPending ? <LoadingState /> : character.isError ? (
        <FailureState error={character.error} onRetry={() => void character.refetch()} />
      ) : (
        <div className="space-y-4">
          <p className="truncate text-sm text-fg-muted">{character.data.draft.profile.name}</p>
          {character.data.archived && <p className="text-sm text-fg-muted">{t("historyPage.archivedCharacter")}</p>}
          <ChatHistoryList
            api={api}
            characterId={characterId}
            busy={busy}
            run={run}
            actionsOpen={actionsOpen}
            onActionsOpenChange={setActionsOpen}
            onOpen={openChat}
            emptyAction={character.data.archived ? undefined : newChat}
          />
        </div>
      )}
    </Page>
  );
}
