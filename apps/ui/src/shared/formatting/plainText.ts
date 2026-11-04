/** Markdown reduced to its words for one-line previews: markers, links, images and tags are dropped, whitespace collapses. */
export function plainText(markdown: string): string {
  return markdown
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/<[^>]+>/g, "")
    .replace(/^\s{0,3}(#{1,6}\s+|>\s?|[-*+]\s+|\d+[.)]\s+)/gm, "")
    .replace(/\*\*|__|~~|`/g, "")
    .replace(/(^|[^\w*])[*_](\S(?:[^*_]*\S)?)[*_](?=[^\w*]|$)/g, "$1$2")
    .replace(/\s+/g, " ")
    .trim();
}
