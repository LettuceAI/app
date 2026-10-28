import type { ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { BookOpen, Pencil } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Checkbox,
  EmptyState,
  icon,
  IconButton,
  ListGroup,
  ListRow,
  LoadingState,
  Sheet,
} from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { FailureState } from "@/shared/ui/FailureState";
import { lorebooksQuery } from "./queries";

/** Any number of lorebooks, attached in the order they were picked. */
export function LorebookChoiceSheet({
  api,
  open,
  value,
  busy = false,
  header,
  onChange,
  onClose,
}: {
  api: ApiClient;
  open: boolean;
  value: string[];
  busy?: boolean;
  header?: ReactNode;
  onChange: (ids: string[]) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation("common");
  const books = useQuery(lorebooksQuery(api));
  const navigate = useNavigate();
  const edit = (lorebookId: string) => {
    onClose();
    void navigate({
      to: "/library/lorebooks/$lorebookId",
      params: { lorebookId },
    });
  };
  const toggle = (id: string, on: boolean) =>
    onChange(on ? [...value, id] : value.filter((entry) => entry !== id));
  return (
    <Sheet
      open={open}
      onClose={onClose}
      dismissible={!busy}
      size="sm"
      title={t("lorebookPicker.title")}
      footer={
        <Button
          variant="primary"
          className="w-full"
          disabled={busy}
          onClick={onClose}
        >
          {t("lorebookPicker.done")}
        </Button>
      }
    >
      <div className="space-y-4 pb-1">
        {header}
        {books.isPending ? (
          <LoadingState />
        ) : books.isError ? (
          <FailureState
            error={books.error}
            onRetry={() => void books.refetch()}
          />
        ) : books.data.length === 0 ? (
          <EmptyState
            size="sm"
            icon={<BookOpen size={icon.xl} />}
            title={t("lorebookPicker.empty")}
            description={t("lorebookPicker.emptyBody")}
          />
        ) : (
          <ListGroup>
            {books.data.map((book) => (
              <ListRow
                key={book.id}
                icon={<BookOpen size={icon.lg} />}
                title={book.name}
                trailing={
                  <span className="flex items-center gap-1">
                    <IconButton
                      label={t("lorebookPicker.edit", { name: book.name })}
                      variant="ghost"
                      disabled={busy}
                      onClick={() => edit(book.id)}
                    >
                      <Pencil size={icon.md} />
                    </IconButton>
                    <Checkbox
                      checked={value.includes(book.id)}
                      disabled={busy}
                      onChange={(on) => toggle(book.id, on)}
                      aria-label={book.name}
                    />
                  </span>
                }
              />
            ))}
          </ListGroup>
        )}
      </div>
    </Sheet>
  );
}
