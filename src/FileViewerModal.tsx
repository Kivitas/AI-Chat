/**
 * FileViewerModal.tsx
 *
 * Read-only modal that displays a file's content with syntax highlighting.
 * Reuses the CodeBlock rendering logic via a raw <pre> approach for simplicity.
 */
import { useEffect, useRef } from "react";
import type { FolderFile } from "./types";

interface Props {
  file: FolderFile;
  onClose: () => void;
}

function detectLang(relPath: string): string {
  const ext = relPath.split(".").pop()?.toLowerCase() ?? "";
  const map: Record<string, string> = {
    ts: "typescript", tsx: "typescript", js: "javascript", jsx: "javascript",
    rs: "rust", py: "python", go: "go", java: "java", cpp: "cpp", c: "c",
    h: "cpp", cs: "csharp", rb: "ruby", php: "php", swift: "swift",
    kt: "kotlin", sh: "bash", bash: "bash", zsh: "bash", ps1: "powershell",
    json: "json", toml: "toml", yaml: "yaml", yml: "yaml", xml: "xml",
    html: "html", htm: "html", css: "css", scss: "css", sql: "sql",
    md: "markdown", lua: "lua", dart: "dart", scala: "scala", r: "r",
    zig: "zig", tf: "hcl", tfvars: "hcl",
  };
  return map[ext] ?? "text";
}

export function FileViewerModal({ file, onClose }: Props) {
  const overlayRef = useRef<HTMLDivElement>(null);

  // Close on backdrop click
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const lang = detectLang(file.relPath);
  const content = file.content ?? "(binary file — content not available)";

  return (
    <div
      className="file-viewer-overlay"
      ref={overlayRef}
      onClick={(e) => { if (e.target === overlayRef.current) onClose(); }}
    >
      <div className="file-viewer-modal">
        <div className="file-viewer-header">
          <span className="file-viewer-path">📄 {file.relPath}</span>
          <div className="file-viewer-actions">
            <button
              type="button"
              className="file-viewer-copy"
              onClick={() => navigator.clipboard.writeText(content).catch(() => {})}
            >
              Copy
            </button>
            <button
              type="button"
              className="file-viewer-close"
              onClick={onClose}
            >
              ✕
            </button>
          </div>
        </div>
        <div className="file-viewer-body">
          <pre className="file-viewer-pre">
            <code className={`language-${lang}`}>
              {content}
            </code>
          </pre>
        </div>
        <div className="file-viewer-footer">
          <span>{file.sizeBytes.toLocaleString()} bytes</span>
          <span>{content.split("\n").length} lines</span>
          <span>{lang}</span>
        </div>
      </div>
    </div>
  );
}
