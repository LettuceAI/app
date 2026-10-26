import DOMPurify from "dompurify";
import { marked } from "marked";

const policy = {
  ALLOWED_TAGS: [
    "a",
    "b",
    "blockquote",
    "br",
    "code",
    "dd",
    "del",
    "details",
    "div",
    "dl",
    "dt",
    "em",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hr",
    "i",
    "img",
    "kbd",
    "li",
    "ol",
    "p",
    "pre",
    "s",
    "small",
    "span",
    "strong",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "th",
    "thead",
    "tr",
    "ul",
  ],
  ALLOWED_ATTR: [
    "href",
    "src",
    "alt",
    "title",
    "width",
    "height",
    "colspan",
    "rowspan",
    "open",
    "rel",
    "target",
    "loading",
    "decoding",
    "referrerpolicy",
  ],
  ALLOW_DATA_ATTR: false,
  ALLOW_ARIA_ATTR: false,
};

export function readmeHtml(markdown: string, documentUrl: string, imageBase: string): string {
  if (!DOMPurify.isSupported) {
    const text = document.createElement("div");
    text.textContent = markdown;
    return text.innerHTML;
  }
  const fragment = DOMPurify.sanitize(marked.parse(markdown, { async: false, gfm: true }), {
    ...policy,
    RETURN_DOM_FRAGMENT: true,
  });
  for (const element of fragment.querySelectorAll("a[href], img[src]")) {
    const image = element.tagName === "IMG";
    const attribute = image ? "src" : "href";
    try {
      const url = new URL(element.getAttribute(attribute) ?? "", image ? imageBase : documentUrl);
      if (!["https:", "http:"].includes(url.protocol) || url.username || url.password) {
        element.removeAttribute(attribute);
        continue;
      }
      element.setAttribute(attribute, url.href);
      if (image) {
        element.setAttribute("loading", "lazy");
        element.setAttribute("decoding", "async");
        element.setAttribute("referrerpolicy", "no-referrer");
      } else {
        element.setAttribute("target", "_blank");
        element.setAttribute("rel", "noopener noreferrer");
      }
    } catch {
      element.removeAttribute(attribute);
    }
  }
  return DOMPurify.sanitize(fragment, policy);
}
