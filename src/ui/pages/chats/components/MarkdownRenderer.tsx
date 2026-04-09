import { useMemo } from "react";
import type { ReactElement, ReactNode } from "react";
import DOMPurify from "dompurify";

type TextColors = {
  texts?: string;
  plain?: string;
  italic?: string;
  quoted?: string;
  code?: string;
};

type MarkdownRendererProps = {
  content: string;
  className?: string;
  onImageClick?: (src: string, alt: string) => void;
  textColors?: TextColors;
};

type ListBuffer = {
  type: "unordered" | "ordered";
  items: string[];
};

function sanitizeUrl(url: string): string | null {
  const clean = DOMPurify.sanitize(`<a href="${url}">x</a>`);
  const match = /href="([^"]*)"/.exec(clean);
  return match?.[1] ?? null;
}

const QUOTE_PAIRS = [
  ['"', '"'],
  ["“", "”"],
  ["„", "”"],
  ["‘", "’"],
  ["‚", "’"],
  ["«", "»"],
  ["‹", "›"],
  ["〝", "〞"],
  ["《", "》"],
  ["「", "」"],
  ["『", "』"],
  ["﹁", "﹂"],
  ["﹃", "﹄"],
] as const;
const QUOTED_TEXT_PATTERN = QUOTE_PAIRS.map(
  ([open, close]) => `${escapeRegExp(open)}[^${escapeRegExp(close)}\\n]+${escapeRegExp(close)}`,
).join("|");
const INLINE_PATTERN = new RegExp(
  `(<img\\s[^>]*>|\\*\\*\\*[^*]+\\*\\*\\*|\\*\\*[^*]+\\*\\*|\\*[^*]+\\*|_[^_]+_|\\\`[^\\\`]+\\\`|${QUOTED_TEXT_PATTERN}|\\[[^\\]]+\\]\\([^)]+\\)|\\[[^\\]]+\\]|\\([^)]+\\)|!\\[[^\\]]*\\]\\([^)]+\\))`,
  "i",
);
const CRLF_PATTERN = /\r\n/g;
const HEADING_PATTERN = /^(#{1,6})\s+(.*)$/;
const QUOTE_PATTERN = /^>\s?/;
const UNORDERED_LIST_PATTERN = /^[-*+]\s+/;
const ORDERED_LIST_PATTERN = /^\d+\.\s+/;
const CODE_FENCE_START = "```";
const IMAGE_PATTERN = /^!\[([^\]]*)\]\(([^)]+)\)$/;
const HTML_IMG_PATTERN = /^<img\s[^>]*>$/i;
const HTML_IMG_SRC = /src=["']([^"']+)["']/i;
const HTML_IMG_ALT = /alt=["']([^"']*?)["']/i;
const HTML_IMG_WIDTH = /width=["']?(\d+)["']?/i;
const HTML_IMG_HEIGHT = /height=["']?(\d+)["']?/i;

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function getQuotePair(token: string): { open: string; close: string } | null {
  for (const [open, close] of QUOTE_PAIRS) {
    if (token.startsWith(open) && token.endsWith(close)) {
      return { open, close };
    }
  }
  return null;
}

function parseImgAttrs(
  tag: string,
): { src: string; alt: string; style: React.CSSProperties } | null {
  const srcMatch = HTML_IMG_SRC.exec(tag);
  if (!srcMatch) return null;
  const src = sanitizeUrl(srcMatch[1]);
  if (!src) return null;

  const altMatch = HTML_IMG_ALT.exec(tag);
  const widthMatch = HTML_IMG_WIDTH.exec(tag);
  const heightMatch = HTML_IMG_HEIGHT.exec(tag);

  const style: React.CSSProperties = { maxWidth: "100%" };
  if (widthMatch) style.width = parseInt(widthMatch[1], 10);
  if (heightMatch) style.height = parseInt(heightMatch[1], 10);

  return { src, alt: altMatch?.[1] ?? "", style };
}

function parseInline(
  text: string,
  keyPrefix: string,
  onImageClick?: (src: string, alt: string) => void,
  textColors?: TextColors,
): ReactNode[] {
  const nodes: ReactNode[] = [];
  let remaining = text;
  let index = 0;

  while (remaining.length > 0) {
    const match = INLINE_PATTERN.exec(remaining);
    if (!match || match.index === undefined) {
      if (remaining) {
        nodes.push(remaining);
      }
      break;
    }

    if (match.index > 0) {
      nodes.push(remaining.slice(0, match.index));
    }

    const token = match[0];
    const afterMatch = remaining.slice(match.index + token.length);
    const key = `${keyPrefix}-${index++}`;
    const quotePair = getQuotePair(token);

    if (token.startsWith("<img") || token.startsWith("<IMG")) {
      const attrs = parseImgAttrs(token);
      if (attrs) {
        const imgEl = (
          <img
            key={key}
            src={attrs.src}
            alt={attrs.alt}
            style={attrs.style}
            className={`rounded-xl ${onImageClick ? "cursor-pointer" : ""}`}
            loading="lazy"
            onClick={onImageClick ? () => onImageClick(attrs.src, attrs.alt) : undefined}
          />
        );
        nodes.push(imgEl);
      }
    } else if (token.startsWith("![")) {
      // Image: ![alt](src)
      const closingBracket = token.indexOf("]");
      const alt = token.slice(2, closingBracket);
      const rawSrc = token.slice(closingBracket + 2, -1);
      const src = sanitizeUrl(rawSrc);
      if (src) {
        nodes.push(
          <img
            key={key}
            src={src}
            alt={alt}
            className={`w-full max-w-md rounded-xl ${onImageClick ? "cursor-pointer" : ""}`}
            loading="lazy"
            onClick={onImageClick ? () => onImageClick(src, alt) : undefined}
          />,
        );
      }
    } else if (token.startsWith("***")) {
      const inner = token.slice(3, -3);
      nodes.push(
        <strong key={key}>
          <em className="opacity-80" style={textColors?.italic ? { color: textColors.italic } : undefined}>
            {parseInline(inner, key, onImageClick, textColors)}
          </em>
        </strong>,
      );
    } else if (token.startsWith("**")) {
      const inner = token.slice(2, -2);
      nodes.push(<strong key={key}>{parseInline(inner, key, onImageClick, textColors)}</strong>);
    } else if (token[0] === "*" || token[0] === "_") {
      const inner = token.slice(1, -1);
      nodes.push(
        <em
          key={key}
          className="opacity-80"
          style={textColors?.italic ? { color: textColors.italic } : undefined}
        >
          {parseInline(inner, key, onImageClick, textColors)}
        </em>,
      );
    } else if (token[0] === "`") {
      nodes.push(
        <code
          key={key}
          className="rounded bg-black/40 px-1 py-0.5"
          style={textColors?.code ? { color: textColors.code } : undefined}
        >
          {token.slice(1, -1)}
        </code>,
      );
    } else if (quotePair) {
      const inner = token.slice(quotePair.open.length, token.length - quotePair.close.length);
      nodes.push(
        <span key={key} style={textColors?.texts ? { color: textColors.texts } : undefined}>
          {quotePair.open}
          {parseInline(inner, key, onImageClick, textColors)}
          {quotePair.close}
        </span>,
      );
    } else if (token[0] === "[" && token.includes("](")) {
      // Link: [label](url)
      const closingBracket = token.indexOf("]");
      const label = token.slice(1, closingBracket);
      const rawUrl = token.slice(closingBracket + 2, -1);
      const url = sanitizeUrl(rawUrl);
      if (url) {
        nodes.push(
          <a
            key={key}
            href={url}
            target="_blank"
            rel="noreferrer"
            className="text-emerald-300 underline underline-offset-2 hover:text-emerald-200"
          >
            {label}
          </a>,
        );
      } else {
        nodes.push(<span key={key}>{label}</span>);
      }
    } else if (token[0] === "[") {
      // Standalone [text] - render as italic with visible brackets
      const inner = token.slice(1, -1);
      nodes.push(
        <em
          key={key}
          className="opacity-80"
          style={textColors?.italic ? { color: textColors.italic } : undefined}
        >
          [{parseInline(inner, key, onImageClick, textColors)}]
        </em>,
      );
    } else if (token[0] === "(") {
      // Standalone (text) - render as italic with visible parentheses
      const inner = token.slice(1, -1);
      nodes.push(
        <em
          key={key}
          className="opacity-80"
          style={textColors?.italic ? { color: textColors.italic } : undefined}
        >
          ({parseInline(inner, key, onImageClick, textColors)})
        </em>,
      );
    }

    remaining = afterMatch;
  }

  return nodes;
}

function flushParagraph(
  buffer: string[],
  nodes: ReactElement[],
  keyIndex: { value: number },
  onImageClick?: (src: string, alt: string) => void,
  textColors?: TextColors,
): void {
  if (buffer.length === 0) return;
  const paragraphText = buffer.join("\n").trim();
  if (!paragraphText) {
    buffer.length = 0;
    return;
  }
  const key = `p-${keyIndex.value++}`;
  nodes.push(
    <p
      key={key}
      className="whitespace-pre-wrap wrap-break-word"
      style={textColors?.plain ? { color: textColors.plain } : undefined}
    >
      {parseInline(paragraphText, key, onImageClick, textColors)}
    </p>,
  );
  buffer.length = 0;
}

function flushList(
  list: ListBuffer | null,
  nodes: ReactElement[],
  keyIndex: { value: number },
  onImageClick?: (src: string, alt: string) => void,
  textColors?: TextColors,
): null {
  if (!list || list.items.length === 0) {
    return null;
  }
  const key = `list-${keyIndex.value++}`;
  const isOrdered = list.type === "ordered";
  const ListTag = isOrdered ? "ol" : "ul";
  const listClass = isOrdered ? "ml-5 space-y-1 list-decimal" : "ml-5 space-y-1 list-disc";

  nodes.push(
    <ListTag key={key} className={listClass}>
      {list.items.map((item, idx) => (
        <li key={idx} className="whitespace-pre-wrap">
          {parseInline(item.trim(), `${key}-${idx}`, onImageClick, textColors)}
        </li>
      ))}
    </ListTag>,
  );
  return null;
}

function flushQuote(
  quoteLines: string[],
  nodes: ReactElement[],
  keyIndex: { value: number },
  onImageClick?: (src: string, alt: string) => void,
  textColors?: TextColors,
): void {
  if (quoteLines.length === 0) return;
  const key = `quote-${keyIndex.value++}`;
  nodes.push(
    <blockquote
      key={key}
      className="border-l-2 border-white/20 pl-4 text-inherit leading-[inherit] italic text-gray-300"
      style={textColors?.quoted ? { color: textColors.quoted } : undefined}
    >
      {quoteLines.map((line, idx) => (
        <p key={idx} className="whitespace-pre-wrap">
          {parseInline(line.trim(), `${key}-${idx}`, onImageClick, textColors)}
        </p>
      ))}
    </blockquote>,
  );
  quoteLines.length = 0;
}

export function MarkdownRenderer({
  content,
  className = "",
  onImageClick,
  textColors,
}: MarkdownRendererProps) {
  const nodes = useMemo(() => {
    const normalized = content.replace(CRLF_PATTERN, "\n");
    const lines = normalized.split("\n");
    const out: ReactElement[] = [];
    const paragraphBuffer: string[] = [];
    const quoteBuffer: string[] = [];
    let listBuffer: ListBuffer | null = null;
    let inCodeBlock = false;
    let codeLang = "";
    const codeLines: string[] = [];
    const keyIndex = { value: 0 };

    const flushAll = () => {
      listBuffer = flushList(listBuffer, out, keyIndex, onImageClick, textColors);
      flushQuote(quoteBuffer, out, keyIndex, onImageClick, textColors);
      flushParagraph(paragraphBuffer, out, keyIndex, onImageClick, textColors);
    };

    for (let i = 0; i < lines.length; i++) {
      const rawLine = lines[i];
      const line = rawLine ?? "";
      const trimmedLine = line.trim();

      // Handle code block start
      if (!inCodeBlock && trimmedLine.startsWith(CODE_FENCE_START)) {
        // Skip malformed fences like ````
        if (trimmedLine.endsWith("````")) continue;

        // Check if it's a single-line code fence that closes itself
        if (trimmedLine !== CODE_FENCE_START && trimmedLine.endsWith(CODE_FENCE_START)) {
          continue;
        }

        flushAll();
        inCodeBlock = true;
        codeLang = trimmedLine.slice(3).trim();
        codeLines.length = 0;
        continue;
      }

      // Handle code block content and end
      if (inCodeBlock) {
        if (trimmedLine === CODE_FENCE_START) {
          const langClass = codeLang ? `language-${codeLang}` : "";
          out.push(
            <pre
              key={`code-${keyIndex.value++}`}
              className="overflow-x-auto rounded-2xl bg-black/70 p-4 text-xs text-emerald-100"
            >
              <code className={langClass}>{codeLines.join("\n")}</code>
            </pre>,
          );
          inCodeBlock = false;
          codeLang = "";
          codeLines.length = 0;
        } else {
          codeLines.push(rawLine);
        }
        continue;
      }

      // Empty line - flush all buffers
      if (trimmedLine === "") {
        flushAll();
        continue;
      }

      // Standalone HTML <img> on its own line
      if (HTML_IMG_PATTERN.test(trimmedLine)) {
        const attrs = parseImgAttrs(trimmedLine);
        if (attrs) {
          flushAll();
          out.push(
            <img
              key={`img-${keyIndex.value++}`}
              src={attrs.src}
              alt={attrs.alt}
              style={attrs.style}
              className={`rounded-xl ${onImageClick ? "cursor-pointer" : ""}`}
              loading="lazy"
              onClick={onImageClick ? () => onImageClick(attrs.src, attrs.alt) : undefined}
            />,
          );
          continue;
        }
      }

      // Standalone image on its own line
      const imageMatch = IMAGE_PATTERN.exec(trimmedLine);
      if (imageMatch) {
        const src = sanitizeUrl(imageMatch[2]);
        if (src) {
          flushAll();
          const alt = imageMatch[1];
          out.push(
            <img
              key={`img-${keyIndex.value++}`}
              src={src}
              alt={alt}
              className={`w-full max-w-md rounded-xl ${onImageClick ? "cursor-pointer" : ""}`}
              loading="lazy"
              onClick={onImageClick ? () => onImageClick(src, alt) : undefined}
            />,
          );
          continue;
        }
      }

      // Headings
      const headingMatch = HEADING_PATTERN.exec(line);
      if (headingMatch) {
        flushAll();
        const level = headingMatch[1].length;
        const HeadingTag = `h${Math.min(level, 6)}` as "h1" | "h2" | "h3" | "h4" | "h5" | "h6";
        const key = `heading-${keyIndex.value++}`;
        out.push(
          <HeadingTag key={key} className="text-inherit leading-[inherit] font-semibold text-white">
            {parseInline(headingMatch[2].trim(), key, onImageClick, textColors)}
          </HeadingTag>,
        );
        continue;
      }

      // Blockquotes
      if (QUOTE_PATTERN.test(line)) {
        listBuffer = flushList(listBuffer, out, keyIndex, onImageClick, textColors);
        paragraphBuffer.length = 0;
        quoteBuffer.push(line.replace(QUOTE_PATTERN, ""));
        continue;
      }

      // Unordered lists
      if (UNORDERED_LIST_PATTERN.test(line)) {
        flushQuote(quoteBuffer, out, keyIndex, onImageClick, textColors);
        paragraphBuffer.length = 0;
        const item = line.replace(UNORDERED_LIST_PATTERN, "");
        if (!listBuffer || listBuffer.type !== "unordered") {
          listBuffer = flushList(listBuffer, out, keyIndex, onImageClick, textColors);
          listBuffer = { type: "unordered", items: [] };
        }
        listBuffer.items.push(item);
        continue;
      }

      // Ordered lists
      if (ORDERED_LIST_PATTERN.test(line)) {
        flushQuote(quoteBuffer, out, keyIndex, onImageClick, textColors);
        paragraphBuffer.length = 0;
        const item = line.replace(ORDERED_LIST_PATTERN, "");
        if (!listBuffer || listBuffer.type !== "ordered") {
          listBuffer = flushList(listBuffer, out, keyIndex, onImageClick, textColors);
          listBuffer = { type: "ordered", items: [] };
        }
        listBuffer.items.push(item);
        continue;
      }

      // Regular paragraph text
      listBuffer = flushList(listBuffer, out, keyIndex, onImageClick, textColors);
      flushQuote(quoteBuffer, out, keyIndex, onImageClick, textColors);
      paragraphBuffer.push(line);
    }

    // Final flush
    flushAll();

    // Handle unclosed code block
    if (inCodeBlock && codeLines.length > 0) {
      const langClass = codeLang ? `language-${codeLang}` : "";
      out.push(
        <pre
          key={`code-${keyIndex.value++}`}
          className="overflow-x-auto rounded-2xl bg-black/70 p-4 text-xs text-emerald-100"
        >
          <code className={langClass}>{codeLines.join("\n")}</code>
        </pre>,
      );
    }

    return out;
  }, [content, onImageClick, textColors]);

  return (
    <div className={`markdown-renderer space-y-3 text-inherit leading-[inherit] ${className}`}>
      {nodes}
    </div>
  );
}
