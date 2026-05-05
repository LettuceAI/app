import React from "react";
import { BookOpen, Check, ChevronRight, Loader2 } from "lucide-react";
import { listLorebooks } from "../../../../core/storage/repo";
import type { Lorebook } from "../../../../core/storage/schemas";
import { BottomMenu } from "../../../components/BottomMenu";
import { cn, interactive, radius, typography } from "../../../design-tokens";

type ActiveLorebooksSelectorProps = {
  selectedIds: string[];
  onChange: (ids: string[]) => void;
  disabled?: boolean;
  subjectLabel?: string;
};

export function ActiveLorebooksSelector({
  selectedIds,
  onChange,
  disabled = false,
  subjectLabel = "character",
}: ActiveLorebooksSelectorProps) {
  const [lorebooks, setLorebooks] = React.useState<Lorebook[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);
  const [open, setOpen] = React.useState(false);

  React.useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    void listLorebooks()
      .then((items) => {
        if (!cancelled) setLorebooks(items);
      })
      .catch((err) => {
        console.error("Failed to load lorebooks:", err);
        if (!cancelled) setError("Failed to load lorebooks");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const selectedSet = React.useMemo(() => new Set(selectedIds), [selectedIds]);
  const selectedLorebooks = React.useMemo(
    () => lorebooks.filter((lorebook) => selectedSet.has(lorebook.id)),
    [lorebooks, selectedSet],
  );
  const selectedCount = selectedIds.length;
  const selectedLabel =
    selectedLorebooks.length > 0
      ? selectedLorebooks.map((lorebook) => lorebook.name || "Untitled lorebook").join(", ")
      : selectedCount > 0
        ? `${selectedCount} active`
        : `Using no ${subjectLabel} lorebooks`;

  const toggleLorebook = React.useCallback(
    (id: string) => {
      if (disabled) return;
      const next = new Set(selectedIds);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      onChange(Array.from(next));
    },
    [disabled, onChange, selectedIds],
  );

  return (
    <>
      <section className="space-y-3">
        <div className="flex items-center gap-2">
          <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
            <BookOpen className="h-4 w-4 text-info" />
          </div>
          <h3 className={cn(typography.body.weight, "text-sm text-fg")}>Active Lorebooks</h3>
          <span className="ml-auto rounded-full border border-fg/10 bg-fg/5 px-2.5 py-1 text-xs text-fg/50">
            {selectedCount} active
          </span>
        </div>

        <button
          type="button"
          disabled={disabled}
          onClick={() => setOpen(true)}
          className={cn(
            "flex w-full items-center gap-3 rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3 text-left",
            interactive.transition.default,
            disabled
              ? "cursor-not-allowed opacity-60"
              : "hover:border-fg/20 hover:bg-surface-el/30 active:scale-[0.99]",
          )}
        >
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm text-fg/80">
              {loading ? "Loading lorebooks..." : error ? error : selectedLabel}
            </p>
            <p className="mt-1 text-xs text-fg/45">
              Sessions inherit these unless a session override is set.
            </p>
          </div>
          {loading ? (
            <Loader2 className="h-4 w-4 shrink-0 animate-spin text-fg/40" />
          ) : (
            <ChevronRight className="h-4 w-4 shrink-0 text-fg/35" />
          )}
        </button>
      </section>

      <BottomMenu
        isOpen={open}
        onClose={() => setOpen(false)}
        title="Active Lorebooks"
        rightAction={
          selectedCount > 0 ? (
            <button
              type="button"
              onClick={() => onChange([])}
              className="rounded-full border border-fg/10 bg-fg/5 px-3 py-1.5 text-xs font-medium text-fg/70 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
            >
              Clear
            </button>
          ) : null
        }
      >
        <div className="space-y-4">
          <p className="text-sm leading-relaxed text-fg/55">
            Choose the lorebooks this {subjectLabel} should activate by default. Existing sessions
            can still override this list.
          </p>

          {loading ? (
            <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-fg/[0.04] px-4 py-3 text-sm text-fg/55">
              <Loader2 className="h-4 w-4 animate-spin" />
              Loading lorebooks...
            </div>
          ) : error ? (
            <div className="rounded-xl border border-danger/20 bg-danger/10 px-4 py-3 text-sm text-danger">
              {error}
            </div>
          ) : lorebooks.length === 0 ? (
            <div className="rounded-xl border border-dashed border-fg/10 bg-fg/[0.03] px-4 py-5 text-sm text-fg/45">
              No lorebooks yet. Create lorebooks from the lorebook manager first.
            </div>
          ) : (
            <div className="space-y-2">
              {lorebooks.map((lorebook) => {
                const selected = selectedSet.has(lorebook.id);
                return (
                  <button
                    key={lorebook.id}
                    type="button"
                    onClick={() => toggleLorebook(lorebook.id)}
                    className={cn(
                      "flex w-full items-center justify-between gap-3 rounded-xl border px-4 py-3 text-left",
                      interactive.transition.default,
                      selected
                        ? "border-info/40 bg-info/10 text-fg"
                        : "border-fg/10 bg-fg/[0.04] text-fg/75 hover:border-fg/20 hover:bg-fg/[0.07]",
                    )}
                  >
                    <span className="min-w-0 truncate text-sm font-medium">
                      {lorebook.name || "Untitled lorebook"}
                    </span>
                    <span
                      className={cn(
                        "flex h-5 w-5 shrink-0 items-center justify-center rounded-full border",
                        radius.full,
                        selected ? "border-info bg-info text-black" : "border-fg/15",
                      )}
                    >
                      {selected && <Check className="h-3.5 w-3.5" />}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </BottomMenu>
    </>
  );
}
