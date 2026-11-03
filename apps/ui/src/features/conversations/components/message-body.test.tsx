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

  it("renders reference images and unlabelled code blocks without enabling HTML", () => {
    const text = '![Harbor][photo]\n\n[photo]: https://example.com/photo.png\n\n```\nhello\n```\n\n<img src="https://example.com/raw.png">';
    const { container } = render(<MessageBody text={text} query="" />);
    expect(screen.getByRole("button", { name: "Harbor" }).querySelector("img")?.getAttribute("src")).toBe("https://example.com/photo.png");
    expect(container.querySelector("pre code")?.textContent).toBe("hello");
    expect(container.querySelector('img[src="https://example.com/raw.png"]')).toBeNull();
  });
});
