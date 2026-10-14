import { useCallback, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate, useRouteContext } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import { startDirectChat } from "./data/actions";

/** Opens a conversation, or a character's latest chat, starting a new one when it has none. */
export function useChatNavigation() {
  const { t } = useTranslation("conversations");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const launching = useRef(false);

  const openConversation = useCallback(
    (conversationId: string) => {
      void navigate({ to: "/chat/$conversationId", params: { conversationId } });
    },
    [navigate],
  );

  const openCharacter = useCallback(
    async (characterId: string, latestId: string | null) => {
      if (latestId) {
        openConversation(latestId);
        return;
      }
      if (launching.current) return;
      launching.current = true;
      try {
        openConversation(await startDirectChat(api, queryClient, characterId));
      } catch {
        toast.error(t("toast.launchFailed"));
      } finally {
        launching.current = false;
      }
    },
    [api, queryClient, openConversation, t],
  );

  return { openConversation, openCharacter };
}
