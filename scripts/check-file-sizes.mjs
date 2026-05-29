#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { existsSync, lstatSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const mode = process.argv[2] ?? "repo";
const maxMb = Number(process.env.MAX_REPO_FILE_SIZE_MB ?? "100");
const maxBytes = maxMb * 1024 * 1024;

const ignoredDirs = new Set([
  ".git",
  "node_modules",
  "dist",
  "dist-ssr",
  "coverage",
]);

const localOnlyRepoFiles = new Set([
  "AI Chat.exe",
]);

const ignoredPathParts = [
  ["src-tauri", "target"],
  ["App", "WebView2Profile"],
  ["Data", "Backups"],
  ["Data", "Chats"],
  ["Data", "Config", "Accounts"],
  ["Data", "Logs"],
  ["Data", "Media"],
  ["Data", "Models"],
  ["Data", "ImageModels"],
  ["Data", "VideoModels"],
  ["Data", "Profiles"],
  ["Runtimes"],
];

const blockedExtensions = new Set([
  ".app",
  ".avi",
  ".bin",
  ".ckpt",
  ".deb",
  ".dll",
  ".dmg",
  ".dylib",
  ".exe",
  ".gguf",
  ".lib",
  ".mkv",
  ".mov",
  ".mp4",
  ".msi",
  ".onnx",
  ".pdb",
  ".pt",
  ".pth",
  ".rpm",
  ".safetensors",
  ".so",
  ".webm",
]);

const textExtensions = new Set([
  ".css",
  ".html",
  ".js",
  ".json",
  ".md",
  ".mjs",
  ".rs",
  ".sh",
  ".toml",
  ".ts",
  ".tsx",
  ".txt",
  ".yaml",
  ".yml",
]);

function rel(file) {
  return path.relative(root, file).replaceAll(path.sep, "/");
}

function hasPathParts(file, parts) {
  const normalized = rel(file).split("/");
  return parts.every((part, index) => normalized[index] === part);
}

function shouldIgnore(file) {
  const relative = rel(file);
  if (!relative || relative.startsWith("..")) return true;
  const parts = relative.split("/");
  if (parts.some((part) => ignoredDirs.has(part))) return true;
  if (relative.endsWith("/.gitkeep") || relative.endsWith("/app-config.sample.json")) return false;
  return ignoredPathParts.some((partsToMatch) => hasPathParts(file, partsToMatch));
}

function listRepoFiles(dir = root, out = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (shouldIgnore(full)) continue;
    if (entry.isDirectory()) {
      listRepoFiles(full, out);
    } else if (entry.isFile()) {
      if (localOnlyRepoFiles.has(rel(full))) continue;
      out.push(full);
    }
  }
  return out;
}

function listStagedFiles() {
  try {
    const output = execFileSync("git", ["diff", "--cached", "--name-only", "-z"], {
      cwd: root,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    return output
      .split("\0")
      .filter(Boolean)
      .map((item) => path.join(root, item))
      .filter((file) => existsSync(file) && lstatSync(file).isFile() && !shouldIgnore(file));
  } catch {
    return listRepoFiles();
  }
}

function isTextFile(file) {
  return textExtensions.has(path.extname(file).toLowerCase());
}

const machineNamePattern = new RegExp(["LAP", "TOP-[A-Z0-9]{8}"].join(""), "i");
const localUserPathPattern = new RegExp(String.raw`[A-Z]:\\Users\\[^\\]+`, "i");
const files = mode === "staged" ? listStagedFiles() : listRepoFiles();
const errors = [];

for (const file of files) {
  const relative = rel(file);
  const extension = path.extname(file).toLowerCase();
  const size = statSync(file).size;

  if (size > maxBytes) {
    errors.push(`${relative}: ${Math.ceil(size / 1024 / 1024)} MB exceeds ${maxMb} MB`);
  }

  if (blockedExtensions.has(extension) && !relative.endsWith(".gitkeep")) {
    errors.push(`${relative}: binary/runtime/model artifact must not be committed`);
  }

  if (isTextFile(file) && size <= 2 * 1024 * 1024) {
    const text = readFileSync(file, "utf8");
    if (machineNamePattern.test(text)) {
      errors.push(`${relative}: contains a machine-specific computer name`);
    }
    if (localUserPathPattern.test(text)) {
      errors.push(`${relative}: contains a machine-specific user path`);
    }
  }
}

if (errors.length) {
  console.error("Repository hygiene check failed:");
  for (const error of errors) console.error(`- ${error}`);
  process.exit(1);
}

console.log(`Repository hygiene check passed (${files.length} files scanned).`);
