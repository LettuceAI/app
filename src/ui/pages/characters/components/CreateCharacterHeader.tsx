import { cn } from "../../../design-tokens";

interface CreateCharacterHeaderProps {
  onBack: () => void;
}

export function CreateCharacterHeader({ onBack }: CreateCharacterHeaderProps) {
  return (
    <header
      className="border-b border-fg/5 bg-surface"
      style={{ paddingTop: "calc(env(safe-area-inset-top) + 8px)" }}
    >
      <div className="relative mx-auto flex h-14 w-full items-center justify-center px-4">
        <button
          onClick={onBack}
          className={cn(
            "absolute left-4 top-1/2 flex -translate-y-1/2 items-center gap-2 px-3 py-1.5",
            "rounded-full border border-fg/15 text-xs font-medium text-fg/80",
            "transition hover:border-fg/30 hover:text-fg active:scale-95"
          )}
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="m15 18-6-6 6-6" />
          </svg>
          Back
        </button>
        <div className="flex flex-col items-center">
          <span className="text-[11px] uppercase tracking-[0.4em] text-fg/40">LettuceAI</span>
          <span className="text-sm font-semibold text-fg">Create</span>
        </div>
      </div>
    </header>
  );
}
