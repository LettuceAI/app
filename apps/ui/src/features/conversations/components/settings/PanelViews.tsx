import { HistoryView, StartersView } from "./ChatViews";
import { LorebooksView } from "./ChoiceViews";
import type { PanelCtx, PanelView } from "./context";
import { GenerationView } from "./GenerationView";
import { NotesView } from "./NotesView";

export function PanelViews({ ctx, current }: { ctx: PanelCtx; current: PanelView }) {
  switch (current) {
    case "lorebooks":
      return <LorebooksView ctx={ctx} />;
    case "history":
      return <HistoryView ctx={ctx} />;
    case "notes":
      return <NotesView ctx={ctx} />;
    case "advanced":
      return <GenerationView ctx={ctx} />;
    case "starters":
      return <StartersView ctx={ctx} />;
    default:
      return null;
  }
}
