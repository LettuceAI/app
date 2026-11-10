import type { WidgetNode } from "./nodes";
import { BoxWidget, ButtonWidget, DividerWidget, ImageWidget, SelectorWidget } from "./ControlWidgets";
import { CharacterInfoWidget, CompanionStateWidget, MemoryWidget, PersonaInfoWidget, SessionInfoWidget } from "./InfoWidgets";
import { AuthorNoteWidget, DiceWidget, QuickSnippetsWidget, ScratchPadWidget, StatTrackerWidget, TimeWidget } from "./InputWidgets";

export function WidgetRenderer({ node }: { node: WidgetNode }) {
  switch (node.type) {
    case "divider":
      return <DividerWidget node={node} />;
    case "box":
      return (
        <BoxWidget node={node}>
          {node.children.map((child) => (
            <WidgetRenderer key={child.id} node={child} />
          ))}
        </BoxWidget>
      );
    case "character_info":
      return <CharacterInfoWidget node={node} />;
    case "persona_info":
      return <PersonaInfoWidget node={node} />;
    case "scratch_pad":
      return <ScratchPadWidget node={node} />;
    case "image":
      return <ImageWidget node={node} />;
    case "selector":
      return <SelectorWidget node={node} />;
    case "button":
      return <ButtonWidget node={node} />;
    case "stat_tracker":
      return <StatTrackerWidget node={node} />;
    case "quick_snippets":
      return <QuickSnippetsWidget node={node} />;
    case "dice":
      return <DiceWidget node={node} />;
    case "memory":
      return <MemoryWidget node={node} />;
    case "companion_state":
      return <CompanionStateWidget node={node} />;
    case "session_info":
      return <SessionInfoWidget node={node} />;
    case "author_note":
      return <AuthorNoteWidget node={node} />;
    case "time":
      return <TimeWidget node={node} />;
  }
}
