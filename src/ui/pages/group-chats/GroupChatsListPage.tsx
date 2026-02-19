import { Trash2, Settings } from "lucide-react";
import { useNavigate } from "react-router-dom";

import { BottomMenu } from "../../components";
import { Routes } from "../../navigation";
import { useGroupChatsListController } from "./hooks/useGroupChatsListController";
import {
  GroupSessionList,
  GroupSessionSkeleton,
  EmptyState,
} from "./components/list/GroupSessionList";

export function GroupChatsListPage() {
  const navigate = useNavigate();
  const {
    groupSessions,
    characters,
    loading,
    selectedSession,
    showDeleteConfirm,
    deleting,
    setSelectedSession,
    setShowDeleteConfirm,
    handleDelete,
  } = useGroupChatsListController();

  const openGroupChat = (session: { id: string }) => {
    navigate(Routes.groupChat(session.id));
  };

  return (
    <div className="flex h-full flex-col pb-6 text-fg/80">
      <main className="flex-1 overflow-y-auto px-1 lg:px-8 pt-4 mx-auto w-full max-w-md lg:max-w-none">
        {loading ? (
          <GroupSessionSkeleton />
        ) : groupSessions.length ? (
          <GroupSessionList
            sessions={groupSessions}
            characters={characters}
            onSelect={openGroupChat}
            onLongPress={setSelectedSession}
          />
        ) : (
          <EmptyState />
        )}
      </main>

      <BottomMenu
        isOpen={Boolean(selectedSession)}
        onClose={() => setSelectedSession(null)}
        includeExitIcon={false}
        title={selectedSession?.name || ""}
      >
        {selectedSession && (
          <div className="space-y-2">
            <button
              onClick={() => {
                navigate(Routes.groupChatSettings(selectedSession.id));
                setSelectedSession(null);
              }}
              className="flex w-full items-center gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition hover:border-fg/20 hover:bg-fg/10"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
                <Settings className="h-4 w-4 text-fg/70" />
              </div>
              <span className="text-sm font-medium text-fg">Edit Group</span>
            </button>

            <button
              onClick={() => {
                setShowDeleteConfirm(true);
              }}
              className="flex w-full items-center gap-3 rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-left transition hover:border-danger/50 hover:bg-danger/20"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full border border-danger/30 bg-danger/20">
                <Trash2 className="h-4 w-4 text-danger" />
              </div>
              <span className="text-sm font-medium text-danger">Delete Group</span>
            </button>
          </div>
        )}
      </BottomMenu>

      <BottomMenu
        isOpen={showDeleteConfirm}
        onClose={() => setShowDeleteConfirm(false)}
        title="Delete Group Chat?"
      >
        <div className="space-y-4">
          <p className="text-sm text-fg/70">
            Are you sure you want to delete "{selectedSession?.name}"? This will also delete all
            messages in this group chat.
          </p>
          <div className="flex gap-3">
            <button
              onClick={() => setShowDeleteConfirm(false)}
              disabled={deleting}
              className="flex-1 rounded-xl border border-fg/10 bg-fg/5 py-3 text-sm font-medium text-fg transition hover:border-fg/20 hover:bg-fg/10 disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              onClick={handleDelete}
              disabled={deleting}
              className="flex-1 rounded-xl border border-danger/30 bg-danger/20 py-3 text-sm font-medium text-danger transition hover:bg-danger/30 disabled:opacity-50"
            >
              {deleting ? "Deleting..." : "Delete"}
            </button>
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}
