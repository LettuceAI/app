import type { GenerationFailureCode, ModelLoadStatus } from "@/api/generated/bindings";
import { sceneDirectiveState, visibleDelta } from "./scene-directive";

export interface ModelLoading {
  model: string;
  percent: number;
  status: ModelLoadStatus;
}

export interface GenerationFailure {
  code: GenerationFailureCode;
  turnId: string;
  targetId: string | null;
}

export interface GenerationMeta {
  active: boolean;
  turnId: string | null;
  userText: string | null;
  userMessageId: string | null;
  targetId: string | null;
  failure: GenerationFailure | null;
}

export interface GenerationText {
  text: string;
  reasoning: string;
  loading: ModelLoading | null;
  imagePrompt: boolean;
}

const idleMeta: GenerationMeta = {
  active: false,
  turnId: null,
  userText: null,
  userMessageId: null,
  targetId: null,
  failure: null,
};

const emptyText: GenerationText = { text: "", reasoning: "", loading: null, imagePrompt: false };

type Listener = () => void;

/**
 * Where one conversation's running generation lives. The page reads `meta`, which changes a handful of
 * times per reply; the streaming bubble alone reads `text`, which changes on every delta and load tick.
 */
export class GenerationStore {
  private meta: GenerationMeta = idleMeta;
  private directive = sceneDirectiveState();
  private text: GenerationText = emptyText;
  private readonly metaListeners = new Set<Listener>();
  private readonly textListeners = new Set<Listener>();

  readonly subscribeMeta = (listener: Listener) => {
    this.metaListeners.add(listener);
    return () => {
      this.metaListeners.delete(listener);
    };
  };

  readonly subscribeText = (listener: Listener) => {
    this.textListeners.add(listener);
    return () => {
      this.textListeners.delete(listener);
    };
  };

  readonly getMeta = () => this.meta;
  readonly getText = () => this.text;

  private setMeta(next: GenerationMeta) {
    this.meta = next;
    for (const listener of [...this.metaListeners]) listener();
  }

  private setText(next: GenerationText) {
    this.text = next;
    for (const listener of [...this.textListeners]) listener();
  }

  private resetText() {
    this.directive = sceneDirectiveState();
    this.setText(emptyText);
  }

  begin(targetId: string | null, userText: string | null) {
    this.resetText();
    this.setMeta({ ...idleMeta, active: true, targetId, userText });
  }

  setTurn(turnId: string) {
    if (this.meta.active && this.meta.turnId !== turnId) this.setMeta({ ...this.meta, turnId });
  }

  setLoading(loading: ModelLoading) {
    if (this.meta.active) this.setText({ ...this.text, loading });
  }

  appendDelta(text: string | null, reasoning: string | null) {
    if (!this.meta.active || (!text && !reasoning)) return;
    this.setText({
      text: this.text.text + (text ? visibleDelta(this.directive, text) : ""),
      reasoning: this.text.reasoning + (reasoning ?? ""),
      loading: text ? null : this.text.loading,
      imagePrompt: this.directive.seen,
    });
  }

  setUserMessage(messageId: string) {
    if (this.meta.active) this.setMeta({ ...this.meta, userMessageId: messageId });
  }

  fail(failure: GenerationFailure) {
    this.resetText();
    this.setMeta({ ...idleMeta, failure });
  }

  dismissFailure() {
    if (this.meta.failure) this.setMeta({ ...this.meta, failure: null });
  }

  clear() {
    this.resetText();
    this.setMeta(idleMeta);
  }
}
