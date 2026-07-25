import { Fragment } from "react";
import { MarkdownRenderer } from "./MarkdownRenderer";
import { SCENE_IMAGE_TOKEN_PATTERN } from "../../../../core/scene/inlineImages";
import { useImageData } from "../../../hooks/useImageData";

type MarkdownProps = React.ComponentProps<typeof MarkdownRenderer>;

interface SceneContentProps {
  content: string;
  className?: string;
  onImageClick?: MarkdownProps["onImageClick"];
  textColors?: MarkdownProps["textColors"];
  highlightRange?: MarkdownProps["highlightRange"];
}

type Segment =
  | { type: "text"; value: string; start: number; end: number }
  | { type: "image"; id: string; ext: string; start: number; end: number };

function parseSegments(content: string): Segment[] {
  const segments: Segment[] = [];
  const pattern = new RegExp(SCENE_IMAGE_TOKEN_PATTERN.source, "g");
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(content)) !== null) {
    if (match.index > lastIndex) {
      segments.push({
        type: "text",
        value: content.slice(lastIndex, match.index),
        start: lastIndex,
        end: match.index,
      });
    }
    segments.push({
      type: "image",
      id: match[1],
      ext: match[2],
      start: match.index,
      end: match.index + match[0].length,
    });
    lastIndex = match.index + match[0].length;
  }
  if (lastIndex < content.length) {
    segments.push({
      type: "text",
      value: content.slice(lastIndex),
      start: lastIndex,
      end: content.length,
    });
  }
  return segments;
}

function relativeHighlightRange(
  segment: Extract<Segment, { type: "text" }>,
  highlightRange?: MarkdownProps["highlightRange"],
): MarkdownProps["highlightRange"] {
  if (!highlightRange) return null;
  const start = Math.max(segment.start, highlightRange.start);
  const end = Math.min(segment.end, highlightRange.end);
  if (end <= start) return null;
  return { start: start - segment.start, end: end - segment.start };
}

function SceneInlineImage({
  id,
  onImageClick,
}: {
  id: string;
  onImageClick?: MarkdownProps["onImageClick"];
}) {
  const url = useImageData(id);
  if (!url) return null;
  return (
    <img
      src={url}
      alt=""
      className={`my-1.5 w-full max-w-md rounded-xl ${onImageClick ? "cursor-pointer" : ""}`}
      loading="lazy"
      onClick={onImageClick ? () => onImageClick(url, "") : undefined}
    />
  );
}

export function SceneContent({
  content,
  className,
  onImageClick,
  textColors,
  highlightRange,
}: SceneContentProps) {
  const segments = parseSegments(content);

  if (segments.length === 1 && segments[0].type === "text") {
    return (
      <MarkdownRenderer
        content={content}
        className={className}
        onImageClick={onImageClick}
        textColors={textColors}
        highlightRange={highlightRange}
      />
    );
  }

  return (
    <>
      {segments.map((segment, index) =>
        segment.type === "image" ? (
          <SceneInlineImage key={`img-${index}`} id={segment.id} onImageClick={onImageClick} />
        ) : segment.value.trim().length > 0 ? (
          <MarkdownRenderer
            key={`text-${index}`}
            content={segment.value}
            className={className}
            onImageClick={onImageClick}
            textColors={textColors}
            highlightRange={relativeHighlightRange(segment, highlightRange)}
          />
        ) : (
          <Fragment key={`empty-${index}`} />
        ),
      )}
    </>
  );
}
