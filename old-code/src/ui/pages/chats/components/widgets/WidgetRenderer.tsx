import type { WidgetNode } from "../../../../../core/storage/schemas";
import { WidgetDivider } from "./WidgetDivider";
import { WidgetBox } from "./WidgetBox";
import { WidgetScratchPad } from "./WidgetScratchPad";
import { WidgetCharacterInfo } from "./WidgetCharacterInfo";
import { WidgetPersonaInfo } from "./WidgetPersonaInfo";
import { WidgetImage } from "./WidgetImage";
import { WidgetButton } from "./WidgetButton";
import { WidgetSelector } from "./WidgetSelector";
import { WidgetStatTracker } from "./WidgetStatTracker";
import { WidgetQuickSnippets } from "./WidgetQuickSnippets";
import { WidgetDice } from "./WidgetDice";
import { WidgetMemory } from "./WidgetMemory";
import { WidgetCompanionState } from "./WidgetCompanionState";
import { WidgetSessionInfo } from "./WidgetSessionInfo";
import { WidgetAuthorNote } from "./WidgetAuthorNote";
import { WidgetTime } from "./WidgetTime";

interface WidgetRendererProps {
  node: WidgetNode;
}

export function WidgetRenderer({ node }: WidgetRendererProps) {
  switch (node.type) {
    case "divider":
      return <WidgetDivider node={node} />;
    case "box":
      return <WidgetBox node={node} />;
    case "scratch_pad":
      return <WidgetScratchPad node={node} />;
    case "character_info":
      return <WidgetCharacterInfo node={node} />;
    case "persona_info":
      return <WidgetPersonaInfo node={node} />;
    case "image":
      return <WidgetImage node={node} />;
    case "selector":
      return <WidgetSelector node={node} />;
    case "button":
      return <WidgetButton node={node} />;
    case "stat_tracker":
      return <WidgetStatTracker node={node} />;
    case "quick_snippets":
      return <WidgetQuickSnippets node={node} />;
    case "dice":
      return <WidgetDice node={node} />;
    case "memory":
      return <WidgetMemory node={node} />;
    case "companion_state":
      return <WidgetCompanionState node={node} />;
    case "session_info":
      return <WidgetSessionInfo node={node} />;
    case "author_note":
      return <WidgetAuthorNote node={node} />;
    case "time":
      return <WidgetTime node={node} />;
  }
}
