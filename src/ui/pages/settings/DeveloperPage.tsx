import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Sparkles, User, MessageSquare, Calculator } from "lucide-react";
import { typography, radius, interactive, cn } from "../../design-tokens";
import {
  saveCharacter,
  savePersona,
  createSession,
  listCharacters,
} from "../../../core/storage/repo";
import type { Character } from "../../../core/storage/schemas";
import { storageBridge } from "../../../core/storage/files";

export function DeveloperPage() {
  const [status, setStatus] = useState<string>("");
  const [error, setError] = useState<string>("");

  const showStatus = (message: string) => {
    setStatus(message);
    setError("");
    setTimeout(() => setStatus(""), 3000);
  };

  const showError = (message: string) => {
    setError(message);
    setStatus("");
  };

  const generateTestCharacter = async () => {
    try {
      const now = Date.now();
      const testCharacter: Partial<Character> = {
        name: "Test Character",
        definition: "A test character created for development purposes.",
        description: "A test character created for development purposes.",
        scenes: [
          {
            id: crypto.randomUUID(),
            content: "A simple test scene for development",
            createdAt: now,
            variants: [],
          },
        ],
      };

      await saveCharacter(testCharacter);
      showStatus("✓ Test character created successfully");
    } catch (err) {
      showError(
        `Failed to create test character: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const generateTestPersona = async () => {
    try {
      const testPersona = {
        title: "Test Persona",
        description: "A test persona for development",
        isDefault: false,
      };

      await savePersona(testPersona);
      showStatus("✓ Test persona created successfully");
    } catch (err) {
      showError(
        `Failed to create test persona: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const generateTestSession = async () => {
    try {
      const characters = await listCharacters();
      if (characters.length === 0) {
        showError("No characters available. Create a test character first.");
        return;
      }

      const character = characters[0];

      const session = await createSession(
        character.id,
        `Test Session - ${new Date().toLocaleTimeString()}`,
        character.scenes?.[0]?.id,
      );

      showStatus(`✓ Test session created: ${session.id}`);
    } catch (err) {
      showError(
        `Failed to create test session: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const generateBulkTestData = async () => {
    try {
      setStatus("Generating bulk test data...");

      for (let i = 1; i <= 3; i++) {
        const now = Date.now();
        const testCharacter: Partial<Character> = {
          name: `Test Character ${i}`,
          definition: `Test character number ${i} for development.`,
          description: `Test character number ${i} for development.`,
          scenes: [
            {
              id: crypto.randomUUID(),
              content: `Test scene ${i} content`,
              createdAt: now,
              variants: [],
            },
          ],
        };
        await saveCharacter(testCharacter);
      }

      for (let i = 1; i <= 2; i++) {
        const testPersona = {
          title: `Test Persona ${i}`,
          description: `Test persona number ${i} for development`,
          isDefault: false,
        };
        await savePersona(testPersona);
      }

      showStatus("✓ Bulk test data created: 3 characters, 2 personas");
    } catch (err) {
      showError(
        `Failed to create bulk test data: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const optimizeDb = async () => {
    try {
      await invoke("db_optimize");
      showStatus("✓ Database optimized");
    } catch (err) {
      showError(`DB optimize failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const backupLegacy = async () => {
    try {
      const result = await invoke<string>("legacy_backup_and_remove");
      showStatus(`✓ ${result}`);
    } catch (err) {
      showError(`Backup failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const recalculateUsageCosts = async () => {
    try {
      setStatus("Recalculating usage costs... This may take a while.");

      // Get OpenRouter API key from settings
      const settings = await storageBridge.readSettings({});
      const openRouterCred = (settings as any)?.providerCredentials?.find(
        (c: any) => c.providerId?.toLowerCase() === "openrouter",
      );

      if (!openRouterCred?.apiKey) {
        showError(
          "OpenRouter API key not found. Please configure it in Settings > Providers first.",
        );
        return;
      }

      const result = await invoke<string>("usage_recalculate_costs", {
        apiKey: openRouterCred.apiKey,
      });
      showStatus(`✓ ${result}`);
    } catch (err) {
      showError(`Recalculation failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  return (
    <div className="flex min-h-screen flex-col bg-surface">
      {/* Content */}
      <main className={cn("flex-1 overflow-auto px-4 py-6")}>
        {/* Status Messages */}
        {status && (
          <div
            className={cn(
              "mb-4 px-4 py-3",
              radius.md,
              "border border-accent/30 bg-accent/10",
              typography.body.size,
              "text-accent/80",
            )}
          >
            {status}
          </div>
        )}

        {error && (
          <div
            className={cn(
              "mb-4 px-4 py-3",
              radius.md,
              "border border-danger/30 bg-danger/10",
              typography.body.size,
              "text-danger/80",
            )}
          >
            {error}
          </div>
        )}

        {/* Test Data Generators */}
        <section className="space-y-3">
          <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg mb-3")}>
            Test Data Generators
          </h2>

          <ActionButton
            icon={<Sparkles />}
            title="Generate Test Character"
            description="Create a single test character"
            onClick={generateTestCharacter}
          />

          <ActionButton
            icon={<User />}
            title="Generate Test Persona"
            description="Create a single test persona"
            onClick={generateTestPersona}
          />

          <ActionButton
            icon={<MessageSquare />}
            title="Generate Test Session"
            description="Create a test chat session with existing character"
            onClick={generateTestSession}
          />

          <ActionButton
            icon={<Sparkles />}
            title="Generate Bulk Test Data"
            description="Create 3 characters and 2 personas"
            onClick={generateBulkTestData}
            variant="primary"
          />
        </section>

        {/* Debug Info */}
        <section className={cn("mt-8 space-y-3")}>
          <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg mb-3")}>
            Storage Maintenance
          </h2>
          <ActionButton
            icon={<Sparkles />}
            title="Optimize Database"
            description="Apply PRAGMAs and run VACUUM (mobile only)"
            onClick={optimizeDb}
            variant="primary"
          />
          <ActionButton
            icon={<Sparkles />}
            title="Backup & Remove Legacy Files"
            description="Moves legacy .bin storage into a backup folder"
            onClick={backupLegacy}
            variant="danger"
          />

          <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg mb-3 mt-6")}>
            Usage Tracking
          </h2>
          <ActionButton
            icon={<Calculator />}
            title="Recalculate All Usage Costs"
            description="Re-fetches pricing and recalculates costs for all OpenRouter usage records"
            onClick={recalculateUsageCosts}
            variant="primary"
          />

          <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg mb-3 mt-6")}>
            Environment Info
          </h2>

          <InfoCard title="Mode" value={import.meta.env.MODE} />

          <InfoCard title="Dev Mode" value={import.meta.env.DEV ? "Yes" : "No"} />

          <InfoCard title="Vite Version" value={import.meta.env.VITE_APP_VERSION || "N/A"} />
        </section>
      </main>
    </div>
  );
}

interface ActionButtonProps {
  icon: React.ReactNode;
  title: string;
  description: string;
  onClick: () => void;
  variant?: "default" | "primary" | "danger";
}

function ActionButton({
  icon,
  title,
  description,
  onClick,
  variant = "default",
}: ActionButtonProps) {
  const variants = {
    default: "border-fg/10 bg-fg/5 hover:border-fg/20 hover:bg-fg/[0.08]",
    primary: "border-info/30 bg-info/10 hover:border-info/50 hover:bg-info/20",
    danger: "border-danger/30 bg-danger/10 hover:border-danger/50 hover:bg-danger/20",
  };

  const iconVariants = {
    default: "border-fg/10 bg-fg/10 text-fg/70",
    primary: "border-info/30 bg-info/20 text-info",
    danger: "border-danger/30 bg-danger/20 text-danger/80",
  };

  return (
    <button
      onClick={onClick}
      className={cn(
        "group w-full px-4 py-3 text-left",
        radius.md,
        "border",
        variants[variant],
        interactive.transition.default,
        interactive.active.scale,
        interactive.focus.ring,
      )}
    >
      <div className="flex items-center gap-3">
        <div
          className={cn(
            "flex h-10 w-10 shrink-0 items-center justify-center",
            radius.md,
            "border",
            interactive.transition.default,
            iconVariants[variant],
          )}
        >
          <span className="[&_svg]:h-5 [&_svg]:w-5">{icon}</span>
        </div>
        <div className="min-w-0 flex-1">
          <div
            className={cn("truncate", typography.body.size, typography.body.weight, "text-fg")}
          >
            {title}
          </div>
          <div className={cn("mt-0.5 line-clamp-1", typography.caption.size, "text-fg/45")}>
            {description}
          </div>
        </div>
      </div>
    </button>
  );
}

interface InfoCardProps {
  title: string;
  value: string;
}

function InfoCard({ title, value }: InfoCardProps) {
  return (
    <div className={cn("px-4 py-3", radius.md, "border border-fg/10 bg-fg/5")}>
      <div className={cn(typography.caption.size, "text-fg/50 mb-1")}>{title}</div>
      <div className={cn(typography.body.size, "text-fg font-mono")}>{value}</div>
    </div>
  );
}
