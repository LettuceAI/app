import { Leaf, ArrowRight } from "lucide-react";

type Props = {
  onNext: () => void;
};

export function WelcomeStep({ onNext }: Props) {
  return (
    <div className="flex flex-col items-center px-4 py-8">
      <div className="flex h-20 w-20 items-center justify-center rounded-3xl border border-emerald-400/30 bg-emerald-500/15">
        <Leaf className="h-10 w-10 text-emerald-300" />
      </div>
      <h1 className="mt-6 text-2xl font-bold text-white">Welcome to Lettuce Engine</h1>
      <p className="mt-2 text-center text-sm text-white/60">
        Let's configure your AI character engine. This will take about 2 minutes.
      </p>
      <div className="mt-8 w-full max-w-sm space-y-3">
        <div className="rounded-xl border border-white/10 bg-white/5 p-4">
          <p className="text-sm text-white/80">
            The Engine gives your AI characters persistent memory, emotions, relationships, and a
            real identity.
          </p>
        </div>
        <div className="rounded-xl border border-white/10 bg-white/5 p-4">
          <p className="text-sm text-white/80">
            First, we'll set up an LLM backend, then configure your engine settings.
          </p>
        </div>
      </div>
      <button
        onClick={onNext}
        className="mt-8 flex items-center gap-2 rounded-full border border-emerald-400/40 bg-emerald-500/20 px-8 py-3 text-sm font-semibold text-emerald-100 transition hover:border-emerald-400/60 hover:bg-emerald-500/30 active:scale-[0.98]"
      >
        Let's Go
        <ArrowRight className="h-4 w-4" />
      </button>
    </div>
  );
}
