import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  abortCompanionSoul,
  generateCompanionSoulDraft,
  type GenerateCompanionSoulRequest,
} from "./soul";
import type { CompanionConfig } from "../storage/schemas";

type SoulGenerationRunRequest = Omit<GenerateCompanionSoulRequest, "requestId">;

function newSoulRequestId(): string {
  const id = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
  return `companion-soul:${id}`;
}

export interface CompanionSoulGeneration {
  generating: boolean;
  liveText: string | null;
  stepTool: string | null;
  generate: (request: SoulGenerationRunRequest) => Promise<Partial<CompanionConfig> | null>;
  abort: () => void;
}

export function useCompanionSoulGeneration(): CompanionSoulGeneration {
  const [generating, setGenerating] = useState(false);
  const [liveText, setLiveText] = useState<string | null>(null);
  const [stepTool, setStepTool] = useState<string | null>(null);
  const requestIdRef = useRef<string | null>(null);
  const abortedRef = useRef(false);

  const generate = useCallback(
    async (request: SoulGenerationRunRequest): Promise<Partial<CompanionConfig> | null> => {
      const requestId = newSoulRequestId();
      requestIdRef.current = requestId;
      abortedRef.current = false;
      setLiveText(null);
      setStepTool(null);
      setGenerating(true);

      const unlisteners: Array<() => void> = [];
      try {
        unlisteners.push(
          await listen("companion-soul:progress", (event: any) => {
            if (event.payload?.requestId === requestId && typeof event.payload?.tool === "string") {
              setStepTool(event.payload.tool);
            }
          }),
          await listen("llm-generation-heartbeat", (event: any) => {
            const id = event.payload?.requestId;
            if (typeof id === "string" && id.startsWith(requestId) && typeof event.payload?.recentText === "string") {
              setLiveText(event.payload.recentText);
            }
          }),
        );

        return await generateCompanionSoulDraft({ ...request, requestId });
      } catch (err) {
        if (abortedRef.current) return null;
        throw err;
      } finally {
        unlisteners.forEach((unlisten) => unlisten());
        requestIdRef.current = null;
        setGenerating(false);
        setLiveText(null);
        setStepTool(null);
      }
    },
    [],
  );

  const abort = useCallback(() => {
    const requestId = requestIdRef.current;
    if (!requestId) return;
    abortedRef.current = true;
    void abortCompanionSoul(requestId).catch(() => {});
  }, []);

  useEffect(() => {
    return () => {
      const requestId = requestIdRef.current;
      if (requestId) {
        abortedRef.current = true;
        void abortCompanionSoul(requestId).catch(() => {});
      }
    };
  }, []);

  return { generating, liveText, stepTool, generate, abort };
}
