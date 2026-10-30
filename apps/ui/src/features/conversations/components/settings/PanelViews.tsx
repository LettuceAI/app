import { HistoryView, StartersView } from "./ChatViews";
import type { PanelCtx, PanelView } from "./context";
import { GenerationView } from "./GenerationView";
import { NotesView } from "./NotesView";
import { GroupAppearanceView } from "./GroupAppearanceView";

export function PanelViews({ ctx, current }: { ctx: PanelCtx; current: PanelView }) {
  switch (current) {
    case "appearance":
      return <GroupAppearanceView ctx={ctx} />;
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
