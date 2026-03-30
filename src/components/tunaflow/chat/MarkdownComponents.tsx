import { lazy, Suspense, useState, useContext, type ComponentPropsWithoutRef, type ReactElement, isValidElement, Children } from "react";
import { copyToClipboard } from "@/lib/clipboard";
import type { Components } from "react-markdown";
import { Check, Copy, ChevronDown, ChevronRight, FileCode } from "lucide-react";
import { cn } from "@/lib/utils";
import { FileViewerContext } from "./fileViewerContext";

// Lazy-load syntax highlighter
const SyntaxHighlighter = lazy(() =>
  import("react-syntax-highlighter").then((mod) => ({ default: mod.Prism }))
);
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";

// ─── Constants ─────────────────────────────────────────────────────────────

/** Auto-collapse threshold (lines) */
const COLLAPSE_THRESHOLD = 15;
/** Visible lines when collapsed */
const COLLAPSED_VISIBLE_LINES = 8;

// ─── Helpers ───────────────────────────────────────────────────────────────

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

// ─── Code block (pre > code) ────────────────────────────────────────────────

function CodeBlock({ children, ...rest }: ComponentPropsWithoutRef<"pre">) {
  const [copied, setCopied] = useState(false);
  const lang = extractLang(children);
  const text = extractText(children);
  const lineCount = text.split("\n").length;
  const shouldCollapse = lineCount > COLLAPSE_THRESHOLD;
  const [expanded, setExpanded] = useState(!shouldCollapse);

  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation();
    copyToClipboard(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div className="relative my-2 rounded-md bg-card/80 border border-border/20 overflow-hidden">
      {/* ─── Header bar ─── */}
      <div className="flex items-center gap-2 px-3 py-1 bg-white/[0.03] border-b border-border/10 text-[10px] text-muted-foreground/50">
        {shouldCollapse && (
          <button
            onClick={() => setExpanded(!expanded)}
            className="flex items-center gap-0.5 hover:text-muted-foreground transition-colors"
          >
            {expanded
              ? <ChevronDown className="w-3 h-3" />
              : <ChevronRight className="w-3 h-3" />
            }
          </button>
        )}
        {lang && <span className="font-mono">{lang}</span>}
        <span>{lineCount} lines</span>
        <div className="flex-1" />
        <button
          onClick={handleCopy}
          className="flex items-center gap-1 hover:text-muted-foreground transition-colors"
        >
          {copied
            ? <><Check className="w-3 h-3 text-status-approved" /><span className="text-status-approved">Copied</span></>
            : <><Copy className="w-3 h-3" /><span>Copy</span></>
          }
        </button>
      </div>

      {/* ─── Code content ─── */}
      <div className="relative">
        <pre
          {...rest}
          className={cn(
            "text-[12px] leading-relaxed overflow-x-auto [&>code]:!bg-transparent [&>code]:!p-0",
            !expanded && "overflow-hidden"
          )}
          style={!expanded ? { maxHeight: `${COLLAPSED_VISIBLE_LINES * 1.6 + 0.75}rem` } : undefined}
        >
          {children}
        </pre>

        {/* Collapse gradient overlay */}
        {!expanded && (
          <div className="absolute bottom-0 left-0 right-0 h-12 bg-gradient-to-t from-card/90 to-transparent flex items-end justify-center pb-1">
            <button
              onClick={() => setExpanded(true)}
              className="text-[10px] text-muted-foreground/60 hover:text-muted-foreground bg-card/80 px-2 py-0.5 rounded border border-border/20 transition-colors"
            >
              Show all {lineCount} lines
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

// ─── File path detection ────────────────────────────────────────────────────

/** Known file extensions for path detection in inline code */
const FILE_EXT_RE = /\.(rs|ts|tsx|js|jsx|py|go|java|rb|md|json|toml|yaml|yml|html|css|sql|sh|bash|xml|c|h|cpp|cc|hpp|vue|svelte|txt|cfg|conf|env|lock|mod|sum)$/;

/** Match patterns like `src/foo/bar.ts` or `src/foo/bar.ts:12` */
const FILE_PATH_RE = /^([a-zA-Z0-9_./-]+\.[a-zA-Z0-9]+)(?::(\d+))?$/;

function parseFilePath(text: string): { path: string; line?: number } | null {
  // Must contain at least one slash or dot to be a path
  if (!text.includes("/") && !text.includes("\\")) return null;
  const m = text.trim().match(FILE_PATH_RE);
  if (!m) return null;
  const filePart = m[1];
  // Must have a known extension
  if (!FILE_EXT_RE.test(filePart)) return null;
  return { path: filePart, line: m[2] ? parseInt(m[2], 10) : undefined };
}

// ─── Inline code / syntax highlighted code ──────────────────────────────────

function InlineCode({ children, className, ...rest }: ComponentPropsWithoutRef<"code">) {
  const fileViewer = useContext(FileViewerContext);
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

  // Check if inline code is a file path
  const text = String(children);
  const fileParsed = parseFilePath(text);

  if (fileParsed && fileViewer) {
    return (
      <code
        {...rest}
        onClick={() => fileViewer.openFile(fileParsed.path, fileParsed.line)}
        className="text-[12px] bg-accent/40 text-primary/80 px-1 py-0.5 rounded cursor-pointer hover:bg-accent/60 hover:text-primary transition-colors inline-flex items-center gap-0.5"
        title={`Open ${fileParsed.path}${fileParsed.line ? `:${fileParsed.line}` : ""}`}
      >
        <FileCode className="w-3 h-3 shrink-0 opacity-50" />
        {children}
      </code>
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
