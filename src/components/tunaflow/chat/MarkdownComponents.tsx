import { lazy, Suspense, useState, type ComponentPropsWithoutRef, type ReactElement, isValidElement, Children } from "react";
import type { Components } from "react-markdown";
import { Check, Copy } from "lucide-react";

// Lazy-load syntax highlighter
const SyntaxHighlighter = lazy(() =>
  import("react-syntax-highlighter").then((mod) => ({ default: mod.Prism }))
);
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";

// ─── Code block (pre > code) ────────────────────────────────────────────────

/** Extract language from <code className="language-xxx"> inside <pre> */
function extractLang(children: React.ReactNode): string | null {
  const child = Children.toArray(children)[0];
  if (isValidElement(child)) {
    const cls = (child as ReactElement<{ className?: string }>).props.className ?? "";
    const m = cls.match(/language-(\w+)/);
    return m ? m[1] : null;
  }
  return null;
}

/** Extract raw text from <code> children */
function extractText(children: React.ReactNode): string {
  const child = Children.toArray(children)[0];
  if (isValidElement(child)) {
    const props = (child as ReactElement<{ children?: React.ReactNode }>).props;
    return String(props.children ?? "").replace(/\n$/, "");
  }
  return "";
}

function CodeBlock({ children, ...rest }: ComponentPropsWithoutRef<"pre">) {
  const [copied, setCopied] = useState(false);
  const lang = extractLang(children);
  const text = extractText(children);

  const handleCopy = () => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <pre {...rest}
      className="group/code relative my-2 rounded-md bg-card/80 text-[12px] leading-relaxed overflow-x-auto border border-border/20 [&>code]:!bg-transparent [&>code]:!p-0">
      {children}
      {/* Copy button — top right */}
      <button onClick={handleCopy} title="Copy"
        className="absolute top-2 right-2 p-1 rounded bg-white/[0.06] hover:bg-white/[0.12] text-sidebar-foreground/40 hover:text-sidebar-foreground transition-all opacity-0 group-hover/code:opacity-100">
        {copied ? <Check className="w-3.5 h-3.5 text-status-approved" /> : <Copy className="w-3.5 h-3.5" />}
      </button>
      {/* Language badge — bottom right */}
      {lang && (
        <span className="absolute bottom-1.5 right-2 text-[9px] font-mono text-sidebar-foreground/25 select-none">
          {lang}
        </span>
      )}
    </pre>
  );
}

// ─── Inline code / syntax highlighted code ──────────────────────────────────

function InlineCode({ children, className, ...rest }: ComponentPropsWithoutRef<"code">) {
  const match = className?.match(/language-(\w+)/);
  if (match) {
    const lang = match[1];
    return (
      <Suspense fallback={<code className={className} {...rest}>{children}</code>}>
        <SyntaxHighlighter
          style={oneDark}
          language={lang}
          PreTag="div"
          customStyle={{
            margin: 0,
            padding: "0.75rem",
            background: "transparent",
            fontSize: "12px",
            lineHeight: "1.6",
          }}
          codeTagProps={{ style: {} }}
        >
          {String(children).replace(/\n$/, "")}
        </SyntaxHighlighter>
      </Suspense>
    );
  }

  return (
    <code {...rest}
      className="text-[12px] bg-accent/40 text-foreground/90 px-1 py-0.5 rounded">
      {children}
    </code>
  );
}

// ─── Table ──────────────────────────────────────────────────────────────────

function ScrollTable({ children, ...rest }: ComponentPropsWithoutRef<"table">) {
  return (
    <div className="overflow-x-auto my-2 rounded-md max-w-full border border-border/20">
      <table {...rest}
        className="w-full border-collapse text-[12px] [&_th]:bg-accent/30 [&_th]:px-2.5 [&_th]:py-1 [&_th]:text-left [&_th]:font-medium [&_th]:text-foreground/70 [&_th]:border-b [&_th]:border-border/20 [&_td]:px-2.5 [&_td]:py-1 [&_td]:border-b [&_td]:border-border/10">
        {children}
      </table>
    </div>
  );
}

// ─── Links ──────────────────────────────────────────────────────────────────

function SafeLink({ href, children, ...rest }: ComponentPropsWithoutRef<"a">) {
  const isExternal = href && (href.startsWith("http://") || href.startsWith("https://"));
  return (
    <a {...rest} href={href}
      {...(isExternal ? { target: "_blank", rel: "noopener noreferrer" } : {})}
      className="text-primary/80 hover:text-primary hover:underline transition-colors">
      {children}
      {isExternal && <span className="inline-block ml-0.5 text-[9px] opacity-30">↗</span>}
    </a>
  );
}

// ─── Blockquote ─────────────────────────────────────────────────────────────

function Quote({ children, ...rest }: ComponentPropsWithoutRef<"blockquote">) {
  return (
    <blockquote {...rest}
      className="my-2 pl-3 border-l-2 border-border/40 text-muted-foreground/70 italic">
      {children}
    </blockquote>
  );
}

// ─── Export ─────────────────────────────────────────────────────────────────

export const markdownComponents: Partial<Components> = {
  pre: CodeBlock as Components["pre"],
  code: InlineCode as Components["code"],
  table: ScrollTable as Components["table"],
  a: SafeLink as Components["a"],
  blockquote: Quote as Components["blockquote"],
};
