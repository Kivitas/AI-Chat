import "katex/dist/katex.min.css";
import { memo, useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";
import type { Components } from "react-markdown";
import rehypeKatex from "rehype-katex";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";

interface Props {
  text: string;
  role: string;
}

// ── Normalise LLM math output to remark-math format ──────────────────────────
// LLMs sometimes emit \[...\], \(...\), or ```latex blocks.
// This converts all of them to $$...$$ and $...$ which remark-math handles.
function normaliseLatex(raw: string): string {
  return raw
    // Some local runtimes double-escape environment delimiters.
    .replace(/\\\\(begin|end)\{/g, "\\$1{")
    // KaTeX expects aligned/gathered inside display math, not top-level align/gather.
    .replace(/\\begin\{align\*?\}/g, "\\begin{aligned}")
    .replace(/\\end\{align\*?\}/g, "\\end{aligned}")
    .replace(/\\begin\{gather\*?\}/g, "\\begin{gathered}")
    .replace(/\\end\{gather\*?\}/g, "\\end{gathered}")
    .replace(/\\begin\{equation\*?\}([\s\S]*?)\\end\{equation\*?\}/g, (_m, math) => `\n\n$$\n${String(math).trim()}\n$$\n\n`)
    // ```latex ... ``` or ```math ... ``` -> display block
    .replace(/```(?:latex|math)\s*([\s\S]*?)```/gi, (_m, math) => `\n\n$$\n${math.trim()}\n$$\n\n`)
    // \[...\] -> display block
    .replace(/\\\[([\s\S]*?)\\\]/g, (_m, math) => `\n\n$$\n${math.trim()}\n$$\n\n`)
    // \(...\) -> inline
    .replace(/\\\(([\s\S]*?)\\\)/g, (_m, math) => `$${String(math).trim()}$`)
    // \begin{equation}...\end{equation} and similar environments -> display block
    .replace(
      /(^|[^$])\\begin\{(aligned|alignedat|gathered|multline\*?|split|cases|array|matrix|pmatrix|bmatrix|Bmatrix|vmatrix|Vmatrix)\}([\s\S]*?)\\end\{\2\}/g,
      (_m, prefix, env, math) => `${prefix}\n\n$$\n\\begin{${env}}${math}\\end{${env}}\n$$\n\n`,
    );
}

// ── Copy button with "Copied!" feedback ──────────────────────────────────────
function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      className="md-code-copy"
      onClick={() => {
        navigator.clipboard.writeText(text).catch(() => {});
        setCopied(true);
        setTimeout(() => setCopied(false), 1400);
      }}
    >
      {copied ? "Copied!" : "Copy"}
    </button>
  );
}

// ── Syntax-highlight using highlight.js on the client ────────────────────────
// We do NOT use rehype-highlight (it adds large overhead and can clash with
// rehype-katex). Instead we apply hljs manually after parsing, which is fast
// and gives us full control over the code-block chrome.
let hljs: typeof import("highlight.js").default | null = null;
let hljsStyleInjected = false;
const HIGHLIGHT_LANGUAGES = [
  "python", "javascript", "typescript", "rust", "cpp", "c", "java", "go",
  "bash", "shell", "sh", "json", "html", "css", "sql", "yaml", "toml",
  "markdown", "r", "latex", "matlab", "haskell", "kotlin", "swift", "php",
  "ruby", "scala", "dart", "lua", "perl", "zig",
];

async function loadHljs() {
  if (hljs) return hljs;
  const mod = await import("highlight.js");
  hljs = mod.default;
  if (!hljsStyleInjected) {
    const link = document.createElement("link");
    link.rel = "stylesheet";
    // Dark GitHub theme matches the code block background (#0d1117)
    link.href = new URL(
      "highlight.js/styles/github-dark.css",
      import.meta.url,
    ).href;
    document.head.appendChild(link);
    hljsStyleInjected = true;
  }
  return hljs;
}

// ── Code block component ──────────────────────────────────────────────────────
function CodeBlock({ lang, code }: { lang: string; code: string }) {
  const highlightKey = `${lang}\u0000${code}`;
  const [highlighted, setHighlighted] = useState<{ key: string; value: string } | null>(null);

  useEffect(() => {
    let cancelled = false;
    if (!code) return;

    void loadHljs().then((hl) => {
      const validLang = lang && HIGHLIGHT_LANGUAGES.includes(lang.toLowerCase()) ? lang.toLowerCase() : null;
      try {
        const result = validLang
          ? hl.highlight(code, { language: validLang, ignoreIllegals: true })
          : hl.highlightAuto(code, HIGHLIGHT_LANGUAGES);
        if (!cancelled) setHighlighted({ key: highlightKey, value: result.value });
      } catch {
        if (!cancelled) setHighlighted(null);
      }
    });

    return () => { cancelled = true; };
  }, [code, highlightKey, lang]);

  const highlightedHtml = highlighted?.key === highlightKey ? highlighted.value : null;

  return (
    <div className="md-code-block">
      <div className="md-code-head">
        <span className="md-code-lang">{lang || "text"}</span>
        <CopyButton text={code} />
      </div>
      <pre>
        {highlightedHtml ? (
          <code
            className={lang ? `language-${lang} hljs` : "hljs"}
            dangerouslySetInnerHTML={{ __html: highlightedHtml }}
          />
        ) : (
          <code className={lang ? `language-${lang}` : ""}>{code}</code>
        )}
      </pre>
    </div>
  );
}

const markdownComponents: Components = {
  code({ children, className }) {
    const match = /language-(\w+)/.exec(className ?? "");
    const lang = match?.[1] ?? "";
    const raw = String(children).replace(/\n$/, "");
    const isBlock = raw.includes("\n") || !!match;

    if (!isBlock) {
      return <code className="md-inline-code">{children}</code>;
    }
    return <CodeBlock lang={lang} code={raw} />;
  },

  pre({ children }) {
    return <>{children}</>;
  },

  table({ children, ...props }) {
    return (
      <div className="table-wrap">
        <table className="md-table" {...props}>{children}</table>
      </div>
    );
  },

  a({ children, href, ...props }) {
    return (
      <a href={href} target="_blank" rel="noopener noreferrer" {...props}>
        {children}
      </a>
    );
  },
};

// ── Main component ────────────────────────────────────────────────────────────
export const MessageRenderer = memo(function MessageRenderer({ text, role }: Props) {
  const processed = normaliseLatex(text);

  return (
    <div className={`msg-rendered msg-rendered-${role}`}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm, remarkMath]}
        rehypePlugins={[rehypeKatex]}
        components={markdownComponents}
      >
        {processed}
      </ReactMarkdown>
    </div>
  );
});
