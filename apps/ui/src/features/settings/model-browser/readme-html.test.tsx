import { describe, expect, it } from "vitest";
import { readmeHtml } from "./readme-html";

const documentUrl = "https://models.example/owner/repo/blob/pin/README.md";
const imageBase = "https://models.example/owner/repo/resolve/pin/README.md";
function render(markdown: string) {
  const result = document.createElement("div");
  result.innerHTML = readmeHtml(markdown, documentUrl, imageBase);
  return result;
}

describe("repository README HTML", () => {
  it("renders mixed Markdown, HTML, tables and relative images and links", () => {
    const html = render(
      `# Model\n\n<div><strong>Details</strong><a href="./guide.md"><img src="./logo.png" width="133" alt="Logo"></a></div>\n\n| Model | Size |\n| --- | --- |\n| Q8 | 12 GB |`,
    );
    expect(html.querySelector("h1")?.textContent).toBe("Model");
    expect(html.querySelector("strong")?.textContent).toBe("Details");
    expect(html.querySelector("table td")?.textContent).toBe("Q8");
    expect(html.querySelector("a")?.getAttribute("href")).toBe(
      "https://models.example/owner/repo/blob/pin/guide.md",
    );
    const image = html.querySelector("img");
    expect(image?.getAttribute("src")).toBe(
      "https://models.example/owner/repo/resolve/pin/logo.png",
    );
    expect(image?.getAttribute("width")).toBe("133");
    expect(image?.getAttribute("loading")).toBe("lazy");
    expect(image?.getAttribute("referrerpolicy")).toBe("no-referrer");
  });

  it("removes executable and embedded markup, dangerous URLs, event handlers and author styles", () => {
    const html = render(
      `<script>alert(1)</script><iframe src="https://evil.example"></iframe><svg onload="alert(1)"></svg><form><input autofocus></form><div style="position:fixed" class="overlay" id="location"><img src="x" onerror="alert(1)"><a href="javascript:alert(1)" onclick="alert(1)">Bad</a><a href="data:text/html,bad">Data</a><img src="data:image/svg+xml,bad"><a href="https://user:pass@evil.example">Credentials</a></div>`,
    );
    expect(html.querySelector("script,iframe,svg,form,input,style,object,embed")).toBeNull();
    expect(html.querySelector("[style],[class],[id],[onerror],[onclick],[onload]")).toBeNull();
    expect([...html.querySelectorAll("a")].every((anchor) => !anchor.hasAttribute("href"))).toBe(
      true,
    );
    expect(
      [...html.querySelectorAll("img")].every(
        (image) => !image.getAttribute("src")?.startsWith("data:"),
      ),
    ).toBe(true);
  });

  it("keeps code samples inert", () => {
    const html = render('```html\n<img src="x" onerror="alert(1)">\n```');
    expect(html.querySelector("img")).toBeNull();
    expect(html.querySelector("pre code")?.textContent).toContain(
      '<img src="x" onerror="alert(1)">',
    );
  });
});
