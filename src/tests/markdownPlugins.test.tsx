/**
 * Regression coverage for `src/lib/markdownPlugins.ts` SSOT.
 *
 * Beta 사용자 보고 (2026-04-26): 채팅/로그 single newline 이 한 줄로 collapse.
 * 해결: `remark-breaks` 플러그인 추가 → single newline → `<br>`.
 *
 * Invariants:
 * - INV-1: paragraph 안 single `\n` → `<br>` 로 분리.
 * - INV-2: 기존 `\n\n` paragraph break 보존.
 * - INV-3: list / code block / table / strikethrough 등 GFM 동작 보존.
 * - INV-4: code block 내부 newline 은 remark-breaks 가 건드리지 않음 (`<pre>` 보존).
 */
import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import ReactMarkdown from "react-markdown";
import { REMARK_PLUGINS } from "@/lib/markdownPlugins";

function renderMd(text: string) {
  return render(<ReactMarkdown remarkPlugins={REMARK_PLUGINS}>{text}</ReactMarkdown>);
}

describe("markdownPlugins SSOT — REMARK_PLUGINS", () => {
  describe("INV-1: single newline preserved as <br>", () => {
    it("converts single newline inside a paragraph to <br>", () => {
      const { container } = renderMd("line one\nline two\nline three");
      const ps = container.querySelectorAll("p");
      // 모두 같은 paragraph 안에 있어야 함 (paragraph break 가 아님)
      expect(ps.length).toBe(1);
      // <br> 두 개가 라인을 분리해야 함
      const brs = container.querySelectorAll("br");
      expect(brs.length).toBeGreaterThanOrEqual(2);
    });

    it("renders multi-line log with brs (Beta 사용자 보고 케이스)", () => {
      const log = [
        "[INFO] starting service",
        "[INFO] listening on port 8080",
        "[ERROR] connection refused",
      ].join("\n");
      const { container } = renderMd(log);
      const text = container.textContent ?? "";
      expect(text).toContain("starting service");
      expect(text).toContain("listening on port 8080");
      expect(text).toContain("connection refused");
      // single newline → 같은 paragraph + <br> 로 분리
      expect(container.querySelectorAll("p").length).toBe(1);
      expect(container.querySelectorAll("br").length).toBeGreaterThanOrEqual(2);
    });
  });

  describe("INV-2: paragraph break (\\n\\n) preserved", () => {
    it("splits double-newline into separate <p> elements", () => {
      const { container } = renderMd("first paragraph\n\nsecond paragraph");
      const ps = container.querySelectorAll("p");
      expect(ps.length).toBe(2);
      expect(ps[0].textContent).toContain("first paragraph");
      expect(ps[1].textContent).toContain("second paragraph");
    });
  });

  describe("INV-3: GFM features preserved", () => {
    it("renders unordered list with <ul>/<li>", () => {
      const md = "- alpha\n- beta\n- gamma";
      const { container } = renderMd(md);
      const ul = container.querySelector("ul");
      expect(ul).not.toBeNull();
      const lis = container.querySelectorAll("li");
      expect(lis.length).toBe(3);
      expect(lis[0].textContent).toContain("alpha");
    });

    it("renders ordered list with <ol>/<li>", () => {
      const md = "1. one\n2. two\n3. three";
      const { container } = renderMd(md);
      const ol = container.querySelector("ol");
      expect(ol).not.toBeNull();
      expect(container.querySelectorAll("li").length).toBe(3);
    });

    it("renders fenced code block with <pre><code> and preserves internal newlines", () => {
      const md = "```js\nconst a = 1;\nconst b = 2;\n```";
      const { container } = renderMd(md);
      const pre = container.querySelector("pre");
      const code = container.querySelector("pre code");
      expect(pre).not.toBeNull();
      expect(code).not.toBeNull();
      // INV-4: code block 내부 newline 은 텍스트로 보존 (paragraph 변환 X)
      const codeText = code?.textContent ?? "";
      expect(codeText).toContain("const a = 1;");
      expect(codeText).toContain("const b = 2;");
      expect(codeText).toContain("\n");
      // code block 안에는 <br> 가 추가되면 안 됨 (remark-breaks 가 건드리지 않음)
      expect(pre?.querySelectorAll("br").length).toBe(0);
    });

    it("renders GFM table", () => {
      const md = "| a | b |\n| --- | --- |\n| 1 | 2 |";
      const { container } = renderMd(md);
      const table = container.querySelector("table");
      expect(table).not.toBeNull();
      expect(container.querySelectorAll("th").length).toBe(2);
      expect(container.querySelectorAll("td").length).toBe(2);
    });

    it("renders GFM strikethrough with double tilde", () => {
      const { container } = renderMd("normal ~~deleted~~ text");
      const del = container.querySelector("del");
      expect(del).not.toBeNull();
      expect(del?.textContent).toBe("deleted");
    });

    it("does NOT treat single tilde as strikethrough (singleTilde: false)", () => {
      const { container } = renderMd("a ~tilde~ word");
      const del = container.querySelector("del");
      expect(del).toBeNull();
      expect(container.textContent).toContain("~tilde~");
    });

    it("renders headings", () => {
      const md = "# H1\n\n## H2\n\n### H3";
      const { container } = renderMd(md);
      expect(container.querySelector("h1")?.textContent).toBe("H1");
      expect(container.querySelector("h2")?.textContent).toBe("H2");
      expect(container.querySelector("h3")?.textContent).toBe("H3");
    });

    it("renders inline code with <code>", () => {
      const { container } = renderMd("inline `code` here");
      const code = container.querySelector("code");
      expect(code?.textContent).toBe("code");
    });
  });

  describe("INV-4: code block isolation from remark-breaks", () => {
    it("does not insert <br> inside fenced code block lines", () => {
      const md = "```\nfirst\nsecond\nthird\n```";
      const { container } = renderMd(md);
      const pre = container.querySelector("pre");
      expect(pre).not.toBeNull();
      // remark-breaks 는 paragraph 자식만 처리 — code block 은 영향 없어야 함
      expect(pre?.querySelectorAll("br").length).toBe(0);
      const codeText = pre?.querySelector("code")?.textContent ?? "";
      expect(codeText.split("\n").filter((l) => l.length > 0).length).toBe(3);
    });
  });

  describe("plugin set sanity", () => {
    it("exports REMARK_PLUGINS as array with 2 entries (gfm + breaks)", () => {
      expect(Array.isArray(REMARK_PLUGINS)).toBe(true);
      expect(REMARK_PLUGINS.length).toBe(2);
    });
  });
});
