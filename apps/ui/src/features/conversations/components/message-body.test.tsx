import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { MessageBody } from "./MessageRow";

afterEach(cleanup);

describe("message Markdown", () => {
  it("keeps quotes, code and embedded images while highlighting search matches", () => {
    const text = 'At the harbor.\n\n> "Wait at the harbor."\n\n```rust\nlet harbor = true;\n\nprintln!("ready");\n```\n\n![Harbor view](https://example.com/harbor.png)';
    const { container } = render(<MessageBody text={text} query="harbor" />);
    expect(container.querySelector("blockquote")?.textContent).toContain('"Wait at the harbor."');
    expect(container.querySelector("blockquote [data-quote] mark")?.textContent).toBe("harbor");
    expect(container.querySelector("pre code")?.textContent).toBe('let harbor = true;\n\nprintln!("ready");');
    expect(screen.getByRole("button", { name: "Copy" })).toBeTruthy();
    const image = screen.getByRole("button", { name: "Harbor view" }).querySelector("img");
    expect(image?.getAttribute("src")).toBe("https://example.com/harbor.png");
    expect(container.querySelectorAll("mark")).toHaveLength(3);
  });

  it("renders reference images, unlabelled code blocks and safe HTML images", () => {
    const text = '![Harbor][photo]\n\n[photo]: https://example.com/photo.png\n\n```\nhello\n```\n\n<img src="https://example.com/raw.png">';
    const { container } = render(<MessageBody text={text} query="" />);
    expect(screen.getByRole("button", { name: "Harbor" }).querySelector("img")?.getAttribute("src")).toBe("https://example.com/photo.png");
    expect(container.querySelector("pre code")?.textContent).toBe("hello");
    expect(container.querySelector('img[src="https://example.com/raw.png"]')).toBeTruthy();
  });

  it("renders styled HTML in the Markdown flow and strips active content", () => {
    const { container } = render(<MessageBody query="harbor" text={'**Before** <span class="fixed" id="app" style="color:#9bd4ff;font-weight:700;position:fixed" onclick="alert(1)">harbor</span> after.\n\n<script>alert(1)</script><iframe src="https://example.com"></iframe>'} />);
    const span = container.querySelector<HTMLSpanElement>('span[style]');
    expect(span?.style.color).toBe("rgb(155, 212, 255)");
    expect(span?.style.fontWeight).toBe("700");
    expect(span?.style.position).toBe("");
    expect(span?.querySelector("mark")?.textContent).toBe("harbor");
    expect(container.querySelector("strong")?.textContent).toBe("Before");
    expect(container.querySelector("script, iframe, #app, .fixed, [onclick]")).toBeNull();
    expect(container.firstElementChild?.className).toContain("[contain:layout_paint]");
  });

  it("keeps disclosures mounted and open while a reply streams", () => {
    const first = '<details><summary>More</summary><span style="color:red">First part</span></details>';
    const { container, rerender } = render(<MessageBody text={first} query="" />);
    const details = container.querySelector("details")!;
    details.open = true;
    rerender(<MessageBody text={`${first}\n\nMore content.`} query="" />);
    expect(container.querySelector("details")).toBe(details);
    expect(details.open).toBe(true);
    expect(container.textContent).toContain("More content.");
  });
});
