/**
 * WorkspacePanel.tsx
 *
 * Collapsible file-tree panel that shows the attached folder context for a chat.
 * Clicking a file opens it in the FileViewerModal.
 */
import { useEffect, useState } from "react";
import { backend } from "./lib/backend";
import type { FolderContext, FolderFile } from "./types";
import { FileViewerModal } from "./FileViewerModal";

interface Props {
  folderPath: string;
  onClose: () => void;
}

/** Build a simple folder tree from the flat list of FolderFiles. */
function buildTree(files: FolderFile[]): TreeNode[] {
  const root: TreeNode[] = [];
  const dirMap = new Map<string, TreeNode>();

  for (const file of files) {
    const parts = file.relPath.split("/");
    let current = root;
    let pathSoFar = "";

    for (let i = 0; i < parts.length - 1; i++) {
      const part = parts[i];
      pathSoFar = pathSoFar ? `${pathSoFar}/${part}` : part;
      if (!dirMap.has(pathSoFar)) {
        const node: TreeNode = { name: part, path: pathSoFar, isDir: true, children: [] };
        dirMap.set(pathSoFar, node);
        current.push(node);
      }
      current = dirMap.get(pathSoFar)!.children!;
    }

    const fileName = parts[parts.length - 1];
    current.push({ name: fileName, path: file.relPath, isDir: false, file });
  }

  return root;
}

interface TreeNode {
  name: string;
  path: string;
  isDir: boolean;
  children?: TreeNode[];
  file?: FolderFile;
}

function TreeItem({
  node,
  depth,
  onFileClick,
}: {
  node: TreeNode;
  depth: number;
  onFileClick: (f: FolderFile) => void;
}) {
  const [open, setOpen] = useState(depth < 2);

  if (node.isDir) {
    return (
      <div className="ws-dir">
        <button
          type="button"
          className="ws-dir-row"
          style={{ paddingLeft: `${8 + depth * 14}px` }}
          onClick={() => setOpen((v) => !v)}
        >
          <span className="ws-icon">{open ? "📂" : "📁"}</span>
          <span className="ws-name">{node.name}/</span>
        </button>
        {open && node.children?.map((child) => (
          <TreeItem key={child.path} node={child} depth={depth + 1} onFileClick={onFileClick} />
        ))}
      </div>
    );
  }

  return (
    <button
      type="button"
      className="ws-file-row"
      style={{ paddingLeft: `${8 + depth * 14}px` }}
      onClick={() => node.file && onFileClick(node.file)}
    >
      <span className="ws-icon">📄</span>
      <span className="ws-name">{node.name}</span>
      {node.file && (
        <span className="ws-size">{formatBytes(node.file.sizeBytes)}</span>
      )}
    </button>
  );
}

function formatBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  return `${(b / (1024 * 1024)).toFixed(1)} MB`;
}

export function WorkspacePanel({ folderPath, onClose }: Props) {
  const [ctx, setCtx] = useState<FolderContext | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedFile, setSelectedFile] = useState<FolderFile | null>(null);

  useEffect(() => {
    let active = true;
    const t = setTimeout(() => {
      setLoading(true);
      setError(null);
      backend
        .readChatFolder(folderPath)
        .then((res) => { if (active) setCtx(res); })
        .catch((e) => { if (active) setError(String(e)); })
        .finally(() => { if (active) setLoading(false); });
    }, 0);
    return () => { active = false; clearTimeout(t); };
  }, [folderPath]);

  const folderName = folderPath.split(/[/\\]/).filter(Boolean).pop() ?? folderPath;
  const tree = ctx ? buildTree(ctx.files) : [];

  return (
    <div className="workspace-panel">
      <div className="workspace-header">
        <span className="workspace-title">
          <span>📁</span>
          <span>{folderName}</span>
        </span>
        <div className="workspace-header-actions">
          {ctx && (
            <span className="workspace-file-count">
              {ctx.totalFiles} file{ctx.totalFiles !== 1 ? "s" : ""}
              {ctx.truncated && " (truncated)"}
            </span>
          )}
          <button
            type="button"
            className="workspace-close"
            onClick={onClose}
            title="Close workspace panel"
          >
            ✕
          </button>
        </div>
      </div>

      <div className="workspace-tree">
        {loading && <div className="workspace-loading">Scanning folder…</div>}
        {error && <div className="workspace-error">⚠ {error}</div>}
        {!loading && !error && ctx && tree.length === 0 && (
          <div className="workspace-empty">No readable files found.</div>
        )}
        {!loading && !error && tree.map((node) => (
          <TreeItem
            key={node.path}
            node={node}
            depth={0}
            onFileClick={setSelectedFile}
          />
        ))}
      </div>

      {selectedFile && (
        <FileViewerModal
          file={selectedFile}
          onClose={() => setSelectedFile(null)}
        />
      )}
    </div>
  );
}
