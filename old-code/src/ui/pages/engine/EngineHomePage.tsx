import { useParams, useNavigate } from "react-router-dom";
import {
  RefreshCw,
  Power,
  Loader2,
  AlertCircle,
  Settings,
  Wrench,
  Activity,
  Plus,
  Trash2,
  MessageCircle,
} from "lucide-react";
import { useEffect, useState } from "react";
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip as RTooltip,
  ResponsiveContainer,
} from "recharts";

import { useEngineHomeController } from "./hooks/useEngineHomeController";
import { useNavigationManager, Routes } from "../../navigation";
import { readSettings } from "../../../core/storage/repo";
import type { ProviderCredential } from "../../../core/storage/schemas";
import type { CharacterActivity } from "../../../core/engine/types";
import { BottomMenu, MenuButton } from "../../components/BottomMenu";
import { useI18n } from "../../../core/i18n/context";

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

type T = ReturnType<typeof useI18n>["t"];
function timeAgo(isoString: string | null, t: T): string {
  if (!isoString) return t("engine.home.never");
  const diff = Date.now() - new Date(isoString).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return t("engine.home.justNow");
  if (mins < 60) return t("engine.home.timeAgo.minutes", { n: mins });
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return t("engine.home.timeAgo.hours", { n: hrs });
  return t("engine.home.timeAgo.days", { n: Math.floor(hrs / 24) });
}

export function EngineHomePage() {
  const { credentialId } = useParams<{ credentialId: string }>();
  const { back } = useNavigationManager();
  const { t } = useI18n();
  const [credential, setCredential] = useState<ProviderCredential | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const settings = await readSettings();
        const cred = settings.providerCredentials.find((p) => p.id === credentialId);
        if (!cancelled && cred) setCredential(cred);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [credentialId]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-white/10 border-t-white/60" />
      </div>
    );
  }

  if (!credential) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-4">
        <p className="text-sm text-white/60">{t("engine.errors.providerNotFound")}</p>
        <button
          onClick={back}
          className="rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-sm text-white/70 hover:bg-white/10"
        >
          {t("common.buttons.goBack")}
        </button>
      </div>
    );
  }

  return <HomeInner credential={credential} />;
}

function HomeInner({ credential }: { credential: ProviderCredential }) {
  const baseUrl = credential.baseUrl || "";
  const apiKey = credential.apiKey || "";
  const navigate = useNavigate();
  const { t } = useI18n();
  const { state, reload, toggleCharacter, selectCharacter, deleteCharacter } =
    useEngineHomeController(baseUrl, apiKey);
  const selectedChar = state.selectedCharacter
    ? state.characters.find((c) => c.slug === state.selectedCharacter)
    : null;

  return (
    <div className="flex h-full flex-col">
      {/* Content */}
      <div className="flex-1 overflow-y-auto space-y-5">
        {/* Status Bar */}
        <div className="flex items-center gap-3 rounded-xl border border-white/10 bg-white/5 px-4 py-3">
          <div
            className={`h-2.5 w-2.5 rounded-full ${state.connected ? "bg-emerald-400" : "bg-rose-400"}`}
          />
          <span className="flex-1 text-sm text-white/80">
            {state.connected ? t("engine.status.connected") : t("engine.status.offline")}
          </span>
          {state.version && (
            <span className="text-xs text-white/40">v{state.version}</span>
          )}
          {state.needsSetup && (
            <button
              onClick={() => navigate(Routes.engineSetup(credential.id))}
              className="rounded-full bg-amber-500/20 px-2.5 py-0.5 text-[10px] font-medium text-amber-300"
            >
              {t("engine.status.needsSetup")}
            </button>
          )}
          <button
            onClick={() => void reload()}
            disabled={state.loading}
            className="rounded-lg p-1.5 transition hover:bg-white/10 disabled:opacity-50"
          >
            <RefreshCw className={`h-4 w-4 text-white/60 ${state.loading ? "animate-spin" : ""}`} />
          </button>
        </div>

        {state.error && (
          <div className="flex items-start gap-2 rounded-xl border border-rose-400/20 bg-rose-500/10 px-4 py-3">
            <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-rose-400" />
            <p className="text-sm text-rose-200">{state.error}</p>
          </div>
        )}

        {/* Characters */}
        <section>
          <div className="mb-2 flex items-center justify-between">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-white/40">
              {t("engine.home.characters")}
            </h2>
            <button
              onClick={() => navigate(Routes.engineCharacterCreate(credential.id))}
              className="flex items-center gap-1 rounded-lg border border-emerald-400/30 bg-emerald-500/15 px-2.5 py-1 text-[11px] font-medium text-emerald-300 transition hover:bg-emerald-500/25"
            >
              <Plus className="h-3 w-3" /> {t("engine.home.newButton")}
            </button>
          </div>
          {state.characters.length === 0 ? (
            <div className="rounded-xl border border-white/10 bg-white/5 p-6 text-center">
              <p className="text-sm text-white/50">{t("engine.home.noCharactersFound")}</p>
            </div>
          ) : (
            <div className="space-y-2">
              {state.characters.map((char) => (
                <div
                  key={char.slug}
                  onClick={() => selectCharacter(char.slug)}
                  className="flex items-center gap-3 rounded-xl border border-white/10 bg-white/5 px-4 py-3 cursor-pointer transition hover:border-white/20"
                >
                  <div
                    className={`h-2.5 w-2.5 shrink-0 rounded-full ${char.loaded ? "bg-emerald-400" : "bg-white/20"}`}
                  />
                  <div className="flex-1 min-w-0">
                    <p className="truncate text-sm font-medium text-white">{char.name}</p>
                    {(char.role || char.era) && (
                      <p className="truncate text-[11px] text-white/50">
                        {[char.role, char.era].filter(Boolean).join(" · ")}
                      </p>
                    )}
                  </div>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      void toggleCharacter(char.slug, char.loaded);
                    }}
                    disabled={state.togglingSlug === char.slug}
                    className={`rounded-lg p-2 transition ${
                      char.loaded
                        ? "bg-emerald-500/15 text-emerald-300 hover:bg-emerald-500/25"
                        : "bg-white/5 text-white/40 hover:bg-white/10 hover:text-white/70"
                    } disabled:opacity-50`}
                  >
                    {state.togglingSlug === char.slug ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <Power className="h-4 w-4" />
                    )}
                  </button>
                </div>
              ))}
            </div>
          )}
        </section>

        {/* Usage Chart */}
        {state.usage && state.usage.characters && state.usage.characters.length > 0 && (
          <section>
            <h2 className="mb-2 text-xs font-semibold uppercase tracking-wider text-white/40">
              {t("engine.home.tokenUsage")}
            </h2>
            <div className="rounded-xl border border-white/10 bg-white/5 p-4">
              <div className="mb-3 flex items-baseline gap-3">
                <span className="text-2xl font-bold text-white">
                  {formatTokens(state.usage.total_tokens)}
                </span>
                <span className="text-xs text-white/40">{t("engine.home.totalTokens")}</span>
              </div>
              <div className="h-40">
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart
                    data={state.usage.characters.map((c) => ({
                      name: c.character.length > 12 ? c.character.slice(0, 12) + "..." : c.character,
                      input: c.total_input_tokens,
                      output: c.total_output_tokens,
                    }))}
                    margin={{ top: 0, right: 0, bottom: 0, left: 0 }}
                  >
                    <XAxis dataKey="name" tick={{ fill: "#fff6", fontSize: 10 }} axisLine={false} tickLine={false} />
                    <YAxis hide />
                    <RTooltip
                      contentStyle={{ background: "#111", border: "1px solid #333", borderRadius: 8, fontSize: 12 }}
                      labelStyle={{ color: "#fff" }}
                    />
                    <Bar dataKey="input" stackId="a" fill="#34d399" radius={[0, 0, 0, 0]} name={t("engine.tokens.input")} />
                    <Bar dataKey="output" stackId="a" fill="#60a5fa" radius={[4, 4, 0, 0]} name={t("engine.tokens.output")} />
                  </BarChart>
                </ResponsiveContainer>
              </div>
              <div className="mt-2 flex items-center gap-4 text-[11px] text-white/50">
                <span className="flex items-center gap-1">
                  <span className="inline-block h-2 w-2 rounded-sm bg-emerald-400" />
                  {t("engine.tokens.input")}
                </span>
                <span className="flex items-center gap-1">
                  <span className="inline-block h-2 w-2 rounded-sm bg-blue-400" />
                  {t("engine.tokens.output")}
                </span>
              </div>
            </div>
          </section>
        )}

        {/* Background Activity */}
        {Object.keys(state.activities).length > 0 && (
          <section>
            <h2 className="mb-2 text-xs font-semibold uppercase tracking-wider text-white/40">
              {t("engine.home.backgroundActivity")}
            </h2>
            {Object.entries(state.activities).map(([slug, activity]) => (
              <ActivityCard key={slug} slug={slug} activity={activity} />
            ))}
          </section>
        )}

        {/* Quick Actions */}
        <section>
          <h2 className="mb-2 text-xs font-semibold uppercase tracking-wider text-white/40">
            {t("engine.home.quickActions")}
          </h2>
          <div className="grid grid-cols-2 gap-2">
            <QuickAction
              icon={<Wrench className="h-4 w-4" />}
              label={t("engine.home.configureProviders")}
              onClick={() => navigate(Routes.engineProvidersConfig(credential.id))}
            />
            <QuickAction
              icon={<Settings className="h-4 w-4" />}
              label={t("engine.home.engineSettings")}
              onClick={() => navigate(Routes.engineSettingsConfig(credential.id))}
            />
          </div>
        </section>
      </div>

      {/* Character Actions Bottom Sheet */}
      <BottomMenu
        isOpen={!!state.selectedCharacter}
        onClose={() => selectCharacter(null)}
        title={selectedChar?.name || t("engine.home.character")}
      >
        {selectedChar && (
          <div className="space-y-4">
            <div className="rounded-lg border border-white/10 bg-white/5 px-3 py-2">
              <p className="truncate text-sm font-medium text-white">{selectedChar.name}</p>
              {(selectedChar.role || selectedChar.era) && (
                <p className="mt-0.5 truncate text-[11px] text-white/50">
                  {[selectedChar.role, selectedChar.era].filter(Boolean).join(" · ")}
                </p>
              )}
            </div>
            {selectedChar.loaded && (
              <MenuButton
                icon={MessageCircle}
                title={t("engine.home.chat")}
                description={t("engine.home.chatDesc")}
                onClick={() => navigate(Routes.engineChat(credential.id, selectedChar.slug))}
                color="from-emerald-500 to-emerald-600"
              />
            )}
            <MenuButton
              icon={Trash2}
              title={state.deletingSlug ? t("engine.home.deletingCharacter") : t("engine.home.deleteCharacter")}
              description={t("engine.home.deleteDesc")}
              onClick={() => void deleteCharacter(selectedChar.slug)}
              disabled={!!state.deletingSlug}
              color="from-rose-500 to-red-600"
            />
          </div>
        )}
      </BottomMenu>
    </div>
  );
}

function ActivityCard({ slug, activity }: { slug: string; activity: CharacterActivity }) {
  const { t } = useI18n();
  const loops = [
    { name: t("engine.activity.synthesis"), data: activity.synthesis },
    { name: t("engine.activity.consolidation"), data: activity.consolidation },
    { name: t("engine.activity.bm25Rebuild"), data: activity.bm25_rebuild },
    { name: t("engine.activity.dripResearch"), data: activity.drip_research },
  ];

  return (
    <div className="rounded-xl border border-white/10 bg-white/5 p-3 mb-2">
      <div className="mb-2 flex items-center gap-2">
        <Activity className="h-3.5 w-3.5 text-white/40" />
        <span className="text-xs font-medium text-white/70">{slug}</span>
        <span
          className={`ml-auto rounded-full px-2 py-0.5 text-[10px] font-medium ${
            activity.loops_running
              ? "bg-emerald-500/20 text-emerald-300"
              : "bg-white/10 text-white/50"
          }`}
        >
          {activity.loops_running ? t("engine.activity.running") : t("engine.activity.stopped")}
        </span>
      </div>
      <div className="space-y-1">
        {loops.map((loop) => (
          <div key={loop.name} className="flex items-center gap-2 text-[11px]">
            <span className="w-24 text-white/50">{loop.name}</span>
            <span
              className={`rounded-full px-1.5 py-0.5 text-[9px] font-medium ${
                loop.data.status === "running"
                  ? "bg-emerald-500/15 text-emerald-300"
                  : "bg-white/10 text-white/40"
              }`}
            >
              {loop.data.status}
            </span>
            <span className="ml-auto text-white/40">{timeAgo(loop.data.last_run, t)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function QuickAction({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="flex items-center gap-2 rounded-xl border border-white/10 bg-white/5 px-3 py-3 text-left transition hover:border-white/20 hover:bg-white/10 active:scale-[0.98]"
    >
      <span className="text-white/50">{icon}</span>
      <span className="text-xs font-medium text-white/70">{label}</span>
    </button>
  );
}
