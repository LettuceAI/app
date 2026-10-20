import { ArrowLeft } from "lucide-react";
import { useCanGoBack, useNavigate, useParams, useRouter } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { icon, IconButton } from "@lettuceai/crisp";

function Placeholder({ page }: { page: "memoryTitle" | "branchesTitle" }) {
  const { t } = useTranslation("conversations");
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const navigate = useNavigate();
  const { conversationId } = useParams({ strict: false });
  const back = () => {
    if (canGoBack) router.history.back();
    else if (conversationId) void navigate({ to: "/chat/$conversationId", params: { conversationId } });
    else void navigate({ to: "/" });
  };
  return (
    <div className="flex h-full min-h-0 flex-col bg-surface">
      <header className="box-content flex h-14 shrink-0 items-center gap-2 border-b border-line px-2 pt-[var(--safe-top)] @xl:px-3">
        <IconButton label={t("header.back")} shape="round" onClick={back}>
          <ArrowLeft size={icon.lg} />
        </IconButton>
        <h1 className="truncate text-base font-semibold text-fg">{t(`placeholder.${page}`)}</h1>
      </header>
    </div>
  );
}

export function ChatMemoryPage() {
  return <Placeholder page="memoryTitle" />;
}

export function ChatBranchesPage() {
  return <Placeholder page="branchesTitle" />;
}
