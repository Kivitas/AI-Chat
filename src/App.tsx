import {
  Archive, ChevronDown, ChevronRight, Cpu, Eye, EyeOff, KeyRound,
  FileUp, Film, Image, Lock, MessageSquare,
  Moon, Palette, PanelLeftClose, Paperclip,
  Pin, Plus, RefreshCw, Search, Send, Settings,
  ShieldOff, Sliders, Sparkles, Star, Sun,
  Trash2, UserPlus, Wrench, X,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, ChangeEvent, MouseEvent as ReactMouseEvent, ReactNode } from "react";
import "./App.css";
import { MessageRenderer } from "./MessageRenderer";
import { backend } from "./lib/backend";
import type { AppSnapshot, BackupRecord, ChatMessage, ChatSettingsUpdatePayload, DiagnosticsReport, MediaAsset, ModelRecord, PathConfig, ProviderStatus, ViewId } from "./types";

type ThemeVar =
  | "--bg"
  | "--surface"
  | "--surface-2"
  | "--surface-3"
  | "--text"
  | "--muted"
  | "--faint"
  | "--border"
  | "--border-strong"
  | "--accent-contrast"
  | "--shadow-sm"
  | "--shadow"
  | "--shadow-lg";
type Theme = {
  id: string;
  name: string;
  mode: "light" | "dark" | "night";
  accent: string;
  vars?: Partial<Record<ThemeVar, string>>;
};
type ThemeGroup = "all" | Theme["mode"];
type GenerationPartialEvent = { chatId: string; text: string; elapsedMs: number };
type MediaGenerationProgressEvent = { chatId: string; kind: MediaAsset["kind"]; provider: string; status: string; progress?: number | null; elapsedMs: number };
type GenerationStats = { chatId: string; elapsedMs: number; outputTokens: number; tps: number };
type GalleryAsset = MediaAsset & { chatTitle: string };
type ProviderKeyField = "openaiKey" | "openrouterKey" | "geminiKey" | "claudeKey" | "mistralKey";
type ProviderDraft = Record<ProviderKeyField, string>;
type ProviderTouched = Record<ProviderKeyField, boolean>;
type ProviderVisibility = Record<ProviderKeyField, boolean>;
type AppearanceSettings = AppSnapshot["config"]["appearance"];

const THEME_VAR_NAMES: ThemeVar[] = [
  "--bg",
  "--surface",
  "--surface-2",
  "--surface-3",
  "--text",
  "--muted",
  "--faint",
  "--border",
  "--border-strong",
  "--accent-contrast",
  "--shadow-sm",
  "--shadow",
  "--shadow-lg",
];

const DEFAULT_APPEARANCE: AppearanceSettings = {
  themeId: "charcoal",
  themeGroup: "dark",
  smooth: true,
  compact: false,
  autoScroll: true,
  typewriter: true,
  showPerformance: true,
  wideMessages: false,
  technicalMode: false,
  scale: 100,
  messageStyle: "cards",
  sidebarCollapsed: false,
};

const THEMES: Theme[] = [
  { id: "graphite", name: "Graphite", mode: "light", accent: "#2f5b56" },
  { id: "paper", name: "Paper", mode: "light", accent: "#3f6b54", vars: { "--bg": "#fbfaf6", "--surface": "rgba(255,255,251,0.95)", "--surface-2": "#f4f1ea", "--surface-3": "#ffffff", "--text": "#1b1a17", "--muted": "#68645d" } },
  { id: "alpine", name: "Alpine", mode: "light", accent: "#356b73", vars: { "--bg": "#f0f6f5", "--surface-2": "#e8f0ef", "--surface-3": "#fbffff" } },
  { id: "clay", name: "Clay", mode: "light", accent: "#8a5643", vars: { "--bg": "#f7f1ed", "--surface-2": "#efe4de", "--text": "#241915", "--muted": "#75645d" } },
  { id: "mono", name: "Mono", mode: "light", accent: "#343a40", vars: { "--bg": "#f5f5f5", "--surface-2": "#eeeeee", "--surface-3": "#ffffff", "--text": "#151515", "--muted": "#5d6266" } },
  { id: "linen", name: "Linen", mode: "light", accent: "#9a623c", vars: { "--bg": "#faf4ea", "--surface-2": "#f0e4d5", "--surface-3": "#fffaf3" } },
  { id: "sage", name: "Sage", mode: "light", accent: "#5b745d", vars: { "--bg": "#f0f5ed", "--surface-2": "#e6eee2", "--surface-3": "#fbfff8" } },
  { id: "frost", name: "Frost", mode: "light", accent: "#3d72a4", vars: { "--bg": "#eef5fb", "--surface-2": "#e4edf5", "--surface-3": "#fbfdff" } },
  { id: "sakura", name: "Sakura", mode: "light", accent: "#b75d77", vars: { "--bg": "#fbf1f5", "--surface-2": "#f3e5eb", "--surface-3": "#fffafd" } },
  { id: "solar", name: "Solar", mode: "light", accent: "#b86f11", vars: { "--bg": "#f9f4e6", "--surface-2": "#eee3c7", "--surface-3": "#fffaf0", "--text": "#242014" } },
  { id: "mint", name: "Mint", mode: "light", accent: "#28845e", vars: { "--bg": "#eff9f3", "--surface-2": "#e1f1e8", "--surface-3": "#fbfffd" } },
  { id: "glacier", name: "Glacier", mode: "light", accent: "#317fa7", vars: { "--bg": "#edf8fb", "--surface-2": "#e0f0f5", "--surface-3": "#fbfeff" } },
  { id: "circuit", name: "Circuit", mode: "light", accent: "#286f6c", vars: { "--bg": "#eef4f2", "--surface-2": "#e3ebe8", "--surface-3": "#fbfffe", "--text": "#111b1a" } },
  { id: "orchid", name: "Orchid", mode: "light", accent: "#875bb4", vars: { "--bg": "#f6f0fb", "--surface-2": "#ece3f3", "--surface-3": "#fffbff" } },
  { id: "studio", name: "Studio", mode: "light", accent: "#4f6178", vars: { "--bg": "#f4f3f0", "--surface-2": "#e8e7e2", "--surface-3": "#fdfcf8" } },
  { id: "porcelain", name: "Porcelain", mode: "light", accent: "#4d6a82", vars: { "--bg": "#f7f8fa", "--surface-2": "#eaedf1", "--surface-3": "#ffffff", "--text": "#171d23", "--muted": "#66707a" } },
  { id: "fjord", name: "Fjord", mode: "light", accent: "#2c7a7b", vars: { "--bg": "#eef7f7", "--surface-2": "#e0efef", "--surface-3": "#fbffff", "--text": "#102224" } },
  { id: "amberglass", name: "Amberglass", mode: "light", accent: "#a56611", vars: { "--bg": "#fbf5ea", "--surface-2": "#f1e7d0", "--surface-3": "#fffdf8", "--text": "#261b0c" } },
  { id: "forest", name: "Forest", mode: "dark", accent: "#7fc8a9", vars: { "--bg": "#101713", "--surface": "rgba(19,28,23,0.95)", "--surface-2": "#17241d", "--surface-3": "#1d2b24" } },
  { id: "charcoal", name: "Charcoal", mode: "dark", accent: "#8eb4ff" },
  { id: "oxide", name: "Oxide", mode: "dark", accent: "#f0a46b", vars: { "--bg": "#171311", "--surface-2": "#241a16", "--surface-3": "#2a211d" } },
  { id: "plum", name: "Plum", mode: "dark", accent: "#d7a5ff", vars: { "--bg": "#151119", "--surface-2": "#201827", "--surface-3": "#291f31" } },
  { id: "nord", name: "Nord", mode: "dark", accent: "#88c0d0", vars: { "--bg": "#111827", "--surface-2": "#172033", "--surface-3": "#1f2a3e", "--text": "#eceff4" } },
  { id: "espresso", name: "Espresso", mode: "dark", accent: "#d2a46b", vars: { "--bg": "#171210", "--surface-2": "#211916", "--surface-3": "#2a211d" } },
  { id: "ink", name: "Ink", mode: "dark", accent: "#a8c7ff", vars: { "--bg": "#10131a", "--surface-2": "#171b25", "--surface-3": "#1f2532" } },
  { id: "ember", name: "Ember", mode: "dark", accent: "#f06a45", vars: { "--bg": "#171111", "--surface-2": "#251817", "--surface-3": "#2d1e1b" } },
  { id: "carbon", name: "Carbon", mode: "dark", accent: "#9fb2c1", vars: { "--bg": "#111315", "--surface-2": "#191d21", "--surface-3": "#20262b" } },
  { id: "copper", name: "Copper", mode: "dark", accent: "#d08b5b", vars: { "--bg": "#161414", "--surface-2": "#211d1b", "--surface-3": "#29231f" } },
  { id: "harbor", name: "Harbor", mode: "dark", accent: "#6ed3d6", vars: { "--bg": "#0f171a", "--surface-2": "#152226", "--surface-3": "#1d2c31" } },
  { id: "moss", name: "Moss", mode: "dark", accent: "#b1c983", vars: { "--bg": "#121711", "--surface-2": "#1a2218", "--surface-3": "#232c20" } },
  { id: "slate", name: "Slate", mode: "dark", accent: "#b8c2cc", vars: { "--bg": "#121416", "--surface-2": "#1a1e22", "--surface-3": "#232930" } },
  { id: "merlot", name: "Merlot", mode: "dark", accent: "#f08ca5", vars: { "--bg": "#171014", "--surface-2": "#23171d", "--surface-3": "#2c1d25" } },
  { id: "voltage", name: "Voltage", mode: "dark", accent: "#79b8ff", vars: { "--bg": "#0f1218", "--surface-2": "#151b24", "--surface-3": "#1d2531", "--text": "#edf4ff" } },
  { id: "noir", name: "AMOLED", mode: "night", accent: "#ffffff", vars: { "--bg": "#000000", "--surface": "rgba(4,4,4,0.97)", "--surface-2": "#060606", "--surface-3": "#0b0b0b", "--text": "#f7f7f7", "--muted": "#a9a9a9", "--faint": "#696969" } },
  { id: "night", name: "Night", mode: "night", accent: "#d7d38f" },
  { id: "midnight", name: "Midnight", mode: "night", accent: "#9bbcff", vars: { "--bg": "#060914", "--surface-2": "#0d1220", "--surface-3": "#141b2c" } },
  { id: "eclipse", name: "Eclipse", mode: "night", accent: "#c4a7ff", vars: { "--bg": "#090713", "--surface-2": "#100d1d", "--surface-3": "#171326" } },
  { id: "aurora", name: "Aurora", mode: "night", accent: "#81f7d8", vars: { "--bg": "#050c0d", "--surface-2": "#0b1516", "--surface-3": "#112022" } },
  { id: "infrared", name: "Infrared", mode: "night", accent: "#ff7d7d", vars: { "--bg": "#100707", "--surface-2": "#180c0d", "--surface-3": "#231112" } },
  { id: "nebula", name: "Nebula", mode: "night", accent: "#d993ff", vars: { "--bg": "#080712", "--surface-2": "#100d1d", "--surface-3": "#191329" } },
  { id: "cyber", name: "Cyber", mode: "night", accent: "#5dffcf", vars: { "--bg": "#020b0c", "--surface-2": "#061516", "--surface-3": "#0b2021" } },
  { id: "terminal-night", name: "Terminal", mode: "night", accent: "#83f28f", vars: { "--bg": "#030604", "--surface-2": "#07100a", "--surface-3": "#0c1810", "--text": "#dfffe5", "--muted": "#92b998" } },
  { id: "lagoon-night", name: "Lagoon", mode: "night", accent: "#75dfc8", vars: { "--bg": "#041011", "--surface-2": "#0a1b1d", "--surface-3": "#102628" } },
  { id: "blueprint", name: "Blueprint", mode: "night", accent: "#7bb2ff", vars: { "--bg": "#051020", "--surface-2": "#0c1b31", "--surface-3": "#142640" } },
  { id: "obsidian", name: "Obsidian", mode: "night", accent: "#c9b891", vars: { "--bg": "#080808", "--surface-2": "#10100f", "--surface-3": "#171715" } },
  { id: "matrix", name: "Matrix", mode: "night", accent: "#66ff8f", vars: { "--bg": "#020704", "--surface-2": "#07110a", "--surface-3": "#0c1b10", "--text": "#e8ffed", "--muted": "#94b79c" } },
  { id: "starlight", name: "Starlight", mode: "night", accent: "#c7d8ff", vars: { "--bg": "#05070d", "--surface-2": "#0b1120", "--surface-3": "#131b2d" } },
  { id: "signal", name: "Signal", mode: "night", accent: "#ffd166", vars: { "--bg": "#080707", "--surface-2": "#120f10", "--surface-3": "#1b1617", "--text": "#fff5da" } },
  { id: "cotton", name: "Cotton", mode: "light", accent: "#4b74a8", vars: { "--bg": "#f9fbff", "--surface-2": "#edf2f8", "--surface-3": "#ffffff", "--text": "#152033", "--muted": "#637083" } },
  { id: "meadow", name: "Meadow", mode: "light", accent: "#4f7d4f", vars: { "--bg": "#f4f8ef", "--surface-2": "#e9f0e1", "--surface-3": "#fcfff7", "--text": "#172116" } },
  { id: "mist", name: "Mist", mode: "light", accent: "#607d8b", vars: { "--bg": "#f3f7f8", "--surface-2": "#e7edef", "--surface-3": "#ffffff", "--muted": "#5f6f75" } },
  { id: "cobalt", name: "Cobalt", mode: "light", accent: "#265fa8", vars: { "--bg": "#f0f5fd", "--surface-2": "#e1eaf7", "--surface-3": "#fbfdff", "--text": "#101a2a" } },
  { id: "coral", name: "Coral", mode: "light", accent: "#bf5b4b", vars: { "--bg": "#fff5f2", "--surface-2": "#f5e7e2", "--surface-3": "#fffdfb", "--text": "#281916" } },
  { id: "violet-paper", name: "Violet Paper", mode: "light", accent: "#7057b8", vars: { "--bg": "#f7f4fb", "--surface-2": "#ebe6f3", "--surface-3": "#fffdff", "--text": "#211c2b" } },
  { id: "seaglass", name: "Sea Glass", mode: "light", accent: "#2f8174", vars: { "--bg": "#edf8f6", "--surface-2": "#dff0ed", "--surface-3": "#fbfffe", "--text": "#10221f" } },
  { id: "pearl", name: "Pearl", mode: "light", accent: "#6c7485", vars: { "--bg": "#fafafa", "--surface-2": "#eeeeef", "--surface-3": "#ffffff", "--text": "#191b20", "--muted": "#606670" } },
  { id: "citrine", name: "Citrine", mode: "light", accent: "#a16f00", vars: { "--bg": "#fbf8ed", "--surface-2": "#eee7cc", "--surface-3": "#fffdf5", "--text": "#221e12" } },
  { id: "basalt", name: "Basalt", mode: "dark", accent: "#9dd1d0", vars: { "--bg": "#101314", "--surface-2": "#171c1e", "--surface-3": "#202729", "--text": "#edf5f4" } },
  { id: "mulberry", name: "Mulberry", mode: "dark", accent: "#db8ebb", vars: { "--bg": "#151014", "--surface-2": "#211820", "--surface-3": "#2a2029", "--text": "#f8eef5" } },
  { id: "pine", name: "Pine", mode: "dark", accent: "#8ccf9f", vars: { "--bg": "#0f1712", "--surface-2": "#162119", "--surface-3": "#1e2b22", "--muted": "#9eb1a5" } },
  { id: "steel", name: "Steel", mode: "dark", accent: "#9ab7d3", vars: { "--bg": "#11161b", "--surface-2": "#18212a", "--surface-3": "#212c36", "--text": "#edf3f8" } },
  { id: "rosewood", name: "Rosewood", mode: "dark", accent: "#e08d7e", vars: { "--bg": "#171211", "--surface-2": "#231918", "--surface-3": "#2c211f" } },
  { id: "graphene", name: "Graphene", mode: "dark", accent: "#b7c4cf", vars: { "--bg": "#0f1012", "--surface-2": "#16181b", "--surface-3": "#1e2226", "--muted": "#9aa3ab" } },
  { id: "reef", name: "Reef", mode: "dark", accent: "#5fd0bd", vars: { "--bg": "#0d1516", "--surface-2": "#132123", "--surface-3": "#1b2b2e", "--text": "#e9fbf7" } },
  { id: "amethyst", name: "Amethyst", mode: "dark", accent: "#b99cff", vars: { "--bg": "#12101a", "--surface-2": "#1a1726", "--surface-3": "#241f33", "--text": "#f3efff" } },
  { id: "void", name: "Void", mode: "night", accent: "#a8b3ff", vars: { "--bg": "#02030a", "--surface-2": "#080a13", "--surface-3": "#101320", "--text": "#f0f2ff", "--muted": "#9ea5c7" } },
  { id: "deep-space", name: "Deep Space", mode: "night", accent: "#79d9ff", vars: { "--bg": "#030712", "--surface-2": "#09111e", "--surface-3": "#101a2a", "--text": "#e9f6ff" } },
  { id: "redline", name: "Redline", mode: "night", accent: "#ff6961", vars: { "--bg": "#080405", "--surface-2": "#12090b", "--surface-3": "#1c0f12", "--text": "#fff1f1" } },
  { id: "tundra-night", name: "Tundra", mode: "night", accent: "#c2e7d9", vars: { "--bg": "#030807", "--surface-2": "#081210", "--surface-3": "#0e1b18", "--text": "#effcf8" } },
  { id: "laser", name: "Laser", mode: "night", accent: "#ff4fd8", vars: { "--bg": "#07030a", "--surface-2": "#100716", "--surface-3": "#1a0d22", "--text": "#fff0fb" } },
  { id: "monolith", name: "Monolith", mode: "night", accent: "#d8d8d8", vars: { "--bg": "#050505", "--surface-2": "#0c0c0c", "--surface-3": "#151515", "--text": "#f3f3f3", "--muted": "#ababab" } },
  { id: "phosphor", name: "Phosphor", mode: "night", accent: "#b7ff5a", vars: { "--bg": "#030703", "--surface-2": "#081007", "--surface-3": "#101a0d", "--text": "#f4ffe9" } },
  { id: "ultraviolet", name: "Ultraviolet", mode: "night", accent: "#9d7cff", vars: { "--bg": "#05030c", "--surface-2": "#0d0918", "--surface-3": "#171027", "--text": "#f2ecff" } },
  { id: "cloudline", name: "Cloudline", mode: "light", accent: "#5177a4", vars: { "--bg": "#f6f8fb", "--surface-2": "#e8edf4", "--surface-3": "#ffffff", "--text": "#182231", "--muted": "#627083" } },
  { id: "terracotta", name: "Terracotta", mode: "light", accent: "#a9573e", vars: { "--bg": "#fbf2ee", "--surface-2": "#f0dfd7", "--surface-3": "#fffaf7", "--text": "#251814", "--muted": "#795f55" } },
  { id: "bluebell", name: "Bluebell", mode: "light", accent: "#5a65b1", vars: { "--bg": "#f4f5fc", "--surface-2": "#e7e9f5", "--surface-3": "#ffffff", "--text": "#191b2d" } },
  { id: "kelp", name: "Kelp", mode: "light", accent: "#357866", vars: { "--bg": "#eff8f3", "--surface-2": "#e1eee8", "--surface-3": "#fbfffd", "--text": "#12221c" } },
  { id: "marigold", name: "Marigold", mode: "light", accent: "#b66f00", vars: { "--bg": "#fbf6e7", "--surface-2": "#f0e5c8", "--surface-3": "#fffdf4", "--text": "#241d10" } },
  { id: "rose-quartz", name: "Rose Quartz", mode: "light", accent: "#b15d7c", vars: { "--bg": "#fbf3f6", "--surface-2": "#f0e1e8", "--surface-3": "#fffafd", "--text": "#261821" } },
  { id: "aqua-paper", name: "Aqua Paper", mode: "light", accent: "#227c8a", vars: { "--bg": "#eef8fa", "--surface-2": "#e0eef2", "--surface-3": "#fbfeff", "--text": "#102126" } },
  { id: "olive-ink", name: "Olive Ink", mode: "light", accent: "#68792e", vars: { "--bg": "#f5f7ee", "--surface-2": "#e8ecd9", "--surface-3": "#fffffb", "--text": "#202414" } },
  { id: "lilac-field", name: "Lilac Field", mode: "light", accent: "#8061b2", vars: { "--bg": "#f7f3fb", "--surface-2": "#eae2f2", "--surface-3": "#fffaff", "--text": "#21182d" } },
  { id: "clearwater", name: "Clearwater", mode: "light", accent: "#3a7ea1", vars: { "--bg": "#f0f8fb", "--surface-2": "#e2eef3", "--surface-3": "#ffffff", "--text": "#12212a" } },
  { id: "ironwood", name: "Ironwood", mode: "dark", accent: "#cf9f76", vars: { "--bg": "#141211", "--surface-2": "#1f1a17", "--surface-3": "#28211d", "--text": "#f5eee9", "--muted": "#b4a096" } },
  { id: "storm", name: "Storm", mode: "dark", accent: "#8fb6df", vars: { "--bg": "#10151b", "--surface-2": "#171f28", "--surface-3": "#202b36", "--text": "#edf3f9" } },
  { id: "juniper", name: "Juniper", mode: "dark", accent: "#8ed0b2", vars: { "--bg": "#0f1614", "--surface-2": "#16211e", "--surface-3": "#1f2b27", "--text": "#eaf8f1" } },
  { id: "burgundy", name: "Burgundy", mode: "dark", accent: "#e28ca0", vars: { "--bg": "#171014", "--surface-2": "#22161c", "--surface-3": "#2b1d24", "--text": "#f9eef2" } },
  { id: "sapphire", name: "Sapphire", mode: "dark", accent: "#7aa9ff", vars: { "--bg": "#0f1420", "--surface-2": "#151d2e", "--surface-3": "#1e2940", "--text": "#edf4ff" } },
  { id: "umber", name: "Umber", mode: "dark", accent: "#c78a58", vars: { "--bg": "#151312", "--surface-2": "#211b17", "--surface-3": "#2a221d", "--text": "#f7efe7" } },
  { id: "jade", name: "Jade", mode: "dark", accent: "#66c39b", vars: { "--bg": "#0d1512", "--surface-2": "#14201a", "--surface-3": "#1c2b23", "--text": "#e9f8ef" } },
  { id: "pewter", name: "Pewter", mode: "dark", accent: "#aeb9c3", vars: { "--bg": "#111416", "--surface-2": "#191e21", "--surface-3": "#22282c", "--text": "#eff2f4" } },
  { id: "magnet", name: "Magnet", mode: "dark", accent: "#ff8fb8", vars: { "--bg": "#121016", "--surface-2": "#1b1722", "--surface-3": "#251f2e", "--text": "#f8eff6" } },
  { id: "canyon-dusk", name: "Canyon Dusk", mode: "dark", accent: "#ef9366", vars: { "--bg": "#16110f", "--surface-2": "#221914", "--surface-3": "#2d211b", "--text": "#fff0e7" } },
  { id: "black-ice", name: "Black Ice", mode: "night", accent: "#9edcff", vars: { "--bg": "#020508", "--surface-2": "#070d12", "--surface-3": "#0e171f", "--text": "#edf8ff", "--muted": "#92a9b8" } },
  { id: "deep-forest", name: "Deep Forest", mode: "night", accent: "#8ff0a4", vars: { "--bg": "#020704", "--surface-2": "#071008", "--surface-3": "#0e1b12", "--text": "#efffed", "--muted": "#92b99b" } },
  { id: "afterhours", name: "Afterhours", mode: "night", accent: "#f0b36a", vars: { "--bg": "#060504", "--surface-2": "#100c08", "--surface-3": "#19130d", "--text": "#fff6e8" } },
  { id: "cold-lab", name: "Cold Lab", mode: "night", accent: "#8fd3ff", vars: { "--bg": "#03070d", "--surface-2": "#09111a", "--surface-3": "#101b28", "--text": "#edf7ff" } },
  { id: "blackberry", name: "Blackberry", mode: "night", accent: "#dc92ff", vars: { "--bg": "#060309", "--surface-2": "#0e0714", "--surface-3": "#180d22", "--text": "#fbf0ff" } },
  { id: "lava", name: "Lava", mode: "night", accent: "#ff6d4d", vars: { "--bg": "#070302", "--surface-2": "#110706", "--surface-3": "#1c0d09", "--text": "#fff1ed" } },
  { id: "deepwater", name: "Deepwater", mode: "night", accent: "#61d6d0", vars: { "--bg": "#020708", "--surface-2": "#061012", "--surface-3": "#0d1b1e", "--text": "#ecffff" } },
  { id: "starforge", name: "Starforge", mode: "night", accent: "#ffc857", vars: { "--bg": "#050506", "--surface-2": "#0e0d0f", "--surface-3": "#171519", "--text": "#fff7df" } },
  { id: "hyperdrive", name: "Hyperdrive", mode: "night", accent: "#6ef3ff", vars: { "--bg": "#02050b", "--surface-2": "#070d18", "--surface-3": "#0e1728", "--text": "#ebfbff" } },
  { id: "velvet", name: "Velvet", mode: "night", accent: "#ff8bb3", vars: { "--bg": "#050205", "--surface-2": "#0e0710", "--surface-3": "#180d1b", "--text": "#fff0f7" } },
];

const VIEWS: Array<{ id: ViewId; label: string; icon: ReactNode }> = [
  { id: "chat",        label: "Chat",        icon: <MessageSquare size={15} /> },
  { id: "models",      label: "Models",      icon: <Wrench size={15} /> },
  { id: "gallery",     label: "Gallery",     icon: <Image size={15} /> },
  { id: "settings",    label: "Settings",    icon: <Settings size={15} /> },
  { id: "backups",     label: "Backups",     icon: <Archive size={15} /> },
  { id: "diagnostics", label: "Diagnostics", icon: <Sparkles size={15} /> },
];

const MAX_ATTACHMENT_BYTES = 128 * 1024 * 1024;

const STARTER_PROMPTS = [
  "Derive the kinematic equations step by step using LaTeX.",
  "Explain and improve this code with a runnable example.",
  "Solve a physics problem with assumptions, units, and checks.",
  "Write a concise answer with tradeoffs and edge cases.",
];

const INSTRUCTION_PRESETS = [
  { label: "Math tutor",    value: "Act as a careful math tutor. Define every symbol, show derivations step by step, use LaTeX for equations ($$...$$ for display, $...$ for inline), and end with a \\boxed{} final result." },
  { label: "Physics tutor", value: "Act as a physics tutor. State assumptions, track units, derive equations step by step with LaTeX, check dimensions, and box the final answer." },
  { label: "Code reviewer", value: "Act as a senior software engineer. Use fenced code blocks with language tags, explain edge cases, include runnable examples, and call out tradeoffs." },
  { label: "Concise",       value: "Answer directly, keep sections short, use Markdown only where it improves readability." },
];

const PROVIDER_FIELDS: Array<{
  field: ProviderKeyField;
  providerId: ProviderStatus["id"];
  label: string;
  defaultModel: string;
}> = [
  { field: "openaiKey", providerId: "openai", label: "OpenAI", defaultModel: "gpt-4.1" },
  { field: "openrouterKey", providerId: "openrouter", label: "OpenRouter", defaultModel: "openrouter/auto" },
  { field: "geminiKey", providerId: "gemini", label: "Gemini", defaultModel: "gemini-2.5-flash" },
  { field: "claudeKey", providerId: "claude", label: "Claude", defaultModel: "claude-sonnet-4-20250514" },
  { field: "mistralKey", providerId: "mistral", label: "Mistral", defaultModel: "mistral-large-latest" },
];

const EMPTY_PROVIDER_DRAFT: ProviderDraft = {
  openaiKey: "",
  openrouterKey: "",
  geminiKey: "",
  claudeKey: "",
  mistralKey: "",
};

const CLEAN_PROVIDER_TOUCHED: ProviderTouched = {
  openaiKey: false,
  openrouterKey: false,
  geminiKey: false,
  claudeKey: false,
  mistralKey: false,
};

const HIDDEN_PROVIDER_KEYS: ProviderVisibility = {
  openaiKey: false,
  openrouterKey: false,
  geminiKey: false,
  claudeKey: false,
  mistralKey: false,
};

// ── Helpers ─────────────────────────────────────────────────────────────────────
function err(e: unknown, fb = "Something went wrong") { return e instanceof Error ? e.message : typeof e === "string" ? e : fb; }
function fmtBytes(b: number) {
  if (!b) return "0 B";
  const u = ["B","KB","MB","GB"], i = Math.min(Math.floor(Math.log(b)/Math.log(1024)), 3);
  return `${(b/1024**i).toFixed(i?1:0)} ${u[i]}`;
}
async function toB64(f: File) {
  return new Promise<string>((resolve, reject) => {
    const r = new FileReader();
    r.onerror = () => reject(new Error("Failed to read file"));
    r.onload = () => {
      const v = typeof r.result === "string" ? r.result : "";
      const b64 = v.split(",", 2)[1];
      if (!b64) reject(new Error("Could not encode file")); else resolve(b64);
    };
    r.readAsDataURL(f);
  });
}
function initial(s?: string | null, fb = "?") { return s?.trim()[0]?.toUpperCase() ?? fb; }
function fmtShortDate(v: string) {
  const d = new Date(v);
  if (Number.isNaN(d.getTime())) return "";
  return d.toDateString() === new Date().toDateString()
    ? d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
    : d.toLocaleDateString([], { month: "short", day: "numeric" });
}
function chatPreview(c: AppSnapshot["chats"][number]) {
  const last = c.messages[c.messages.length - 1];
  if (!last) return "No messages yet";
  return last.text.replace(/\s+/g, " ").trim().slice(0, 80) || "Attachment";
}
function estimateTokens(s: string) { return Math.max(0, Math.ceil(s.trim().length / 4)); }
function nextTitle(chats: AppSnapshot["chats"]) {
  const nums = new Set(chats.map(c => c.title.match(/^Chat\s+(\d+)$/i)?.[1]).filter(Boolean).map(Number));
  let n = 1; while (nums.has(n)) n++; return `Chat ${n}`;
}
function defaultNewForm(title: string, modelId = "") {
  return { title, modelId, userName: "You", aiName: "AI Companion", instructions: "Use clean Markdown, show derivations step by step with LaTeX, and use fenced code blocks with language labels." };
}
function settingsPayload(chat: AppSnapshot["chats"][number], patch: Partial<ChatSettingsUpdatePayload> = {}): ChatSettingsUpdatePayload {
  return {
    chatId: chat.id,
    modelId: patch.modelId !== undefined ? patch.modelId : (chat.settings.modelId ?? null),
    imageModelId: patch.imageModelId !== undefined ? patch.imageModelId : (chat.settings.imageModelId ?? null),
    videoModelId: patch.videoModelId !== undefined ? patch.videoModelId : (chat.settings.videoModelId ?? null),
    profileOverrideId: patch.profileOverrideId !== undefined ? patch.profileOverrideId : (chat.settings.profileOverrideId ?? null),
    userName: patch.userName !== undefined ? patch.userName : (chat.settings.userName ?? null),
    aiName: patch.aiName !== undefined ? patch.aiName : (chat.settings.aiName ?? null),
    aiInstructions: patch.aiInstructions !== undefined ? patch.aiInstructions : (chat.settings.aiInstructions ?? null),
    userAvatarBase64: patch.userAvatarBase64 ?? null,
    aiAvatarBase64: patch.aiAvatarBase64 ?? null,
    userAvatarPath: patch.userAvatarPath !== undefined ? patch.userAvatarPath : (chat.settings.userAvatar ?? null),
    aiAvatarPath: patch.aiAvatarPath !== undefined ? patch.aiAvatarPath : (chat.settings.aiAvatar ?? null),
  };
}
function clampMenu(v: number, size: number, vp: number) { return Math.max(10, Math.min(v, Math.max(10, vp - size - 10))); }
function sleep(ms: number) { return new Promise(resolve => setTimeout(resolve, ms)); }
function themeSwatchStyle(themeOption: Theme): CSSProperties {
  const surface = themeOption.vars?.["--surface-3"]
    ?? (themeOption.mode === "light" ? "#ffffff" : themeOption.mode === "dark" ? "#222120" : "#111113");
  const bg = themeOption.vars?.["--bg"]
    ?? (themeOption.mode === "light" ? "#f5f4f1" : themeOption.mode === "dark" ? "#131312" : "#09090b");
  return { background: `linear-gradient(135deg, ${bg} 0 34%, ${surface} 34% 68%, ${themeOption.accent} 68% 100%)` };
}

// ── Collapsible ─────────────────────────────────────────────────────────────────
function Collapsible({ title, icon, children, open: def = true }: { title: string; icon?: ReactNode; children: ReactNode; open?: boolean }) {
  const [open, setOpen] = useState(def);
  return (
    <section className="collapsible">
      <button className="collapse-trigger" type="button" onClick={() => setOpen(v => !v)}>
        <span className="with-icon">{icon}{title}</span>
        <ChevronDown size={14} className={open ? "chevron open" : "chevron"} />
      </button>
      {open && <div className="collapse-body">{children}</div>}
    </section>
  );
}

function pickPreferredChatModel(models: ModelRecord[]) {
  const local = [...models]
    .filter(m => m.available && !m.missing && !m.id.startsWith("provider:"))
    .sort((left, right) => left.sizeBytes - right.sizeBytes || left.name.localeCompare(right.name))[0];
  if (local) return local;
  const provider = models.find(m => m.id.startsWith("provider:") && m.available && !m.missing);
  if (provider) return provider;
  return models[0] ?? null;
}

function MediaCard({ asset }: { asset: GalleryAsset }) {
  const src = backend.toAssetUrl(asset.thumbnailPath ?? asset.path);
  const isPending = asset.path.startsWith("pending://") || asset.status === "pending" || asset.status === "failed";
  return (
    <div className="media-card">
      <div className="preview-thumb">
        {asset.kind === "image" && src && !isPending ? (
          <img src={src} alt={asset.sourceLabel ?? "Image attachment"} />
        ) : asset.kind === "video" && src && !isPending ? (
          <video src={src} controls preload="metadata" />
        ) : asset.kind === "file" && asset.contentPreview ? (
          <pre>{asset.contentPreview.replace(/^Extracted text from .*?:\n/, "").slice(0, 260)}</pre>
        ) : asset.kind === "video" ? (
          <Film size={22} />
        ) : asset.kind === "file" ? (
          <FileUp size={22} />
        ) : (
          <Image size={22} />
        )}
      </div>
      <strong>{asset.sourceLabel ?? asset.prompt ?? asset.kind}</strong>
      <span>{asset.chatTitle}{asset.status ? ` · ${asset.status}` : ""}</span>
      {asset.prompt && <small>{asset.prompt}</small>}
    </div>
  );
}

// ── Diagnostics View ───────────────────────────────────────────────────────────
function MessageAttachmentPreview({ asset }: { asset: MediaAsset }) {
  const src = backend.toAssetUrl(asset.thumbnailPath ?? asset.path);
  const status = asset.status ?? "ready";
  const isPending = asset.path.startsWith("pending://") || status === "pending";
  const isFailed = status === "failed";
  const label = asset.sourceLabel ?? (asset.kind === "image" ? "Image" : asset.kind === "video" ? "Video" : "File");
  const prompt = asset.prompt ?? asset.contentPreview ?? "";
  return (
    <div className={`message-attachment ${asset.kind} ${isPending ? "pending" : ""}${isFailed ? " failed" : ""}`}>
      <div className="message-attachment-preview">
        {asset.kind === "image" && src && !isPending && !isFailed ? (
          <img src={src} alt={label} />
        ) : asset.kind === "video" && src && !isPending && !isFailed ? (
          <video src={src} controls preload="metadata" />
        ) : asset.kind === "file" && asset.contentPreview ? (
          <pre>{asset.contentPreview.replace(/^Extracted text from .*?:\n/, "").slice(0, 320)}</pre>
        ) : (
          <div className="generation-placeholder">
            {asset.kind === "video" ? <Film size={24} /> : asset.kind === "image" ? <Image size={24} /> : <FileUp size={24} />}
            {isPending && <span className="generation-spinner" aria-hidden="true" />}
          </div>
        )}
      </div>
      <div className="message-attachment-meta">
        <strong>{label}</strong>
        <span>{isPending ? "Generating" : isFailed ? "Failed" : "Ready"}{asset.modelUsed ? ` · ${asset.modelUsed}` : ""}</span>
        {prompt && <small>{prompt}</small>}
        {isPending && <div className="generation-bar" aria-hidden="true"><i /></div>}
      </div>
    </div>
  );
}

function SparkLine({ values, color = "var(--accent)", height = 40 }: { values: number[]; color?: string; height?: number }) {
  if (values.length < 2) return null;
  const max = Math.max(...values, 1);
  const w = 200; const h = height;
  const pts = values.map((v, i) => `${(i / (values.length - 1)) * w},${h - (v / max) * (h - 4) - 2}`);
  const area = `M${pts[0]} ` + pts.slice(1).map(p => `L${p}`).join(" ") + ` L${w},${h} L0,${h} Z`;
  const line = `M${pts[0]} ` + pts.slice(1).map(p => `L${p}`).join(" ");
  return (
    <svg viewBox={`0 0 ${w} ${h}`} width="100%" height={h} preserveAspectRatio="none" style={{ display: "block" }}>
      <defs><linearGradient id={`sg-${color.replace(/[^a-z0-9]/gi,"")}`} x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stopColor={color} stopOpacity={0.25} /><stop offset="100%" stopColor={color} stopOpacity={0} /></linearGradient></defs>
      <path d={area} fill={`url(#sg-${color.replace(/[^a-z0-9]/gi,"")})`} />
      <path d={line} fill="none" stroke={color} strokeWidth={1.5} strokeLinejoin="round" strokeLinecap="round" />
    </svg>
  );
}

function DonutChart({ pct, color = "var(--accent)", size = 72, label }: { pct: number; color?: string; size?: number; label?: string }) {
  const r = size * 0.38; const c = size / 2;
  const circ = 2 * Math.PI * r;
  const dash = (Math.min(pct, 100) / 100) * circ;
  return (
    <div style={{ position: "relative", width: size, height: size, flexShrink: 0 }}>
      <svg width={size} height={size} style={{ transform: "rotate(-90deg)" }}>
        <circle cx={c} cy={c} r={r} fill="none" stroke="var(--border-strong)" strokeWidth={size * 0.115} />
        <circle cx={c} cy={c} r={r} fill="none" stroke={color} strokeWidth={size * 0.115} strokeDasharray={`${dash} ${circ - dash}`} strokeLinecap="round" style={{ transition: "stroke-dasharray 600ms ease" }} />
      </svg>
      {label && <div style={{ position: "absolute", inset: 0, display: "grid", placeItems: "center", fontSize: size * 0.17, fontWeight: 600, fontFamily: "monospace" }}>{label}</div>}
    </div>
  );
}

function BarChart({ bars, unit = "", color = "var(--accent)" }: { bars: { label: string; value: number; max: number }[]; unit?: string; color?: string }) {
  return (
    <div style={{ display: "grid", gap: 8 }}>
      {bars.map(b => (
        <div key={b.label} style={{ display: "grid", gridTemplateColumns: "88px 1fr 52px", gap: 8, alignItems: "center" }}>
          <span style={{ fontSize: "0.72rem", color: "var(--muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{b.label}</span>
          <div style={{ height: 7, borderRadius: 999, background: "var(--border-strong)", overflow: "hidden" }}>
            <div style={{ height: "100%", borderRadius: 999, width: `${Math.min((b.value / b.max) * 100, 100)}%`, background: color, transition: "width 500ms ease", minWidth: b.value > 0 ? 3 : 0 }} />
          </div>
          <span style={{ fontSize: "0.7rem", fontFamily: "monospace", color: "var(--text)", textAlign: "right" }}>{b.value.toLocaleString()}{unit ? ` ${unit}` : ""}</span>
        </div>
      ))}
    </div>
  );
}

function SegmentedBar({ segments }: { segments: { label: string; value: number; color: string }[] }) {
  const total = Math.max(segments.reduce((sum, item) => sum + item.value, 0), 1);
  return (
    <div className="diag-segment-wrap">
      <div className="diag-segment-bar">
        {segments.filter(item => item.value > 0).map(item => (
          <span
            key={item.label}
            style={{ width: `${Math.max(4, (item.value / total) * 100)}%`, background: item.color }}
            title={`${item.label}: ${item.value.toLocaleString()}`}
          />
        ))}
      </div>
      <div className="diag-segment-legend">
        {segments.map(item => (
          <span key={item.label}><i style={{ background: item.color }} />{item.label} <strong>{item.value.toLocaleString()}</strong></span>
        ))}
      </div>
    </div>
  );
}

function MiniHistogram({ values, color = "var(--accent)", unit = "" }: { values: number[]; color?: string; unit?: string }) {
  const max = Math.max(...values, 1);
  return (
    <div className="diag-histogram">
      {values.map((value, index) => (
        <span
          key={`${value}-${index}`}
          style={{ height: `${Math.max(8, (value / max) * 100)}%`, background: color }}
          title={`${value.toLocaleString()}${unit ? ` ${unit}` : ""}`}
        />
      ))}
    </div>
  );
}

const HISTORY_MAX = 30;

function DiagnosticsView({ snap, diag, setDiag, lastStats, generationHistory, sysMeta, technicalMode, setToast }:
  { snap: AppSnapshot; diag: DiagnosticsReport | null; setDiag: (d: DiagnosticsReport) => void;
    lastStats: GenerationStats | null; generationHistory: GenerationStats[];
    sysMeta: { cores: number; ramGb: number }; technicalMode: boolean; setToast: (s: string) => void }) {

  const [refreshing,     setRefreshing]     = useState(false);
  const tpsHistory     = generationHistory.map(item => item.tps);
  const latencyHistory = generationHistory.map(item => item.elapsedMs / 1000);
  const tokenHistory   = generationHistory.map(item => item.outputTokens);

  // Auto-load on first open
  useEffect(() => {
    if (diag) return;
    backend.diagnostics().then(setDiag).catch(e => setToast(err(e)));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function refresh() {
    setRefreshing(true);
    try { setDiag(await backend.diagnostics()); } catch (e) { setToast(err(e)); } finally { setRefreshing(false); }
  }

  const allModels = diag ? [...diag.detectedModels] : [];
  const totalModelBytes = allModels.reduce((s, m) => s + (m.sizeBytes ?? 0), 0);
  const modelBytesByKind = ["chat", "image", "video"].map(kind => ({
    label: kind,
    value: allModels.filter(model => model.kind === kind).reduce((sum, model) => sum + (model.sizeBytes ?? 0), 0),
  }));
  const modelCountByKind = ["chat", "image", "video"].map(kind => ({
    label: kind,
    value: allModels.filter(model => model.kind === kind).length,
  }));
  const totalChats = snap.chats.length;
  const totalMsgs  = snap.chats.reduce((s, c) => s + c.messages.length, 0);
  const avgMsgs    = totalChats ? Math.round(totalMsgs / totalChats) : 0;
  const totalTokens = snap.chats.reduce((s, c) => s + c.messages.reduce((ms, m) => ms + estimateTokens(m.text), 0), 0);
  const roleCounts = ["user", "assistant", "media"].map(role => ({
    label: role,
    value: snap.chats.reduce((sum, chat) => sum + chat.messages.filter(message => message.role === role).length, 0),
  }));
  const chatsByMsg = [...snap.chats].sort((a,b) => b.messages.length - a.messages.length).slice(0, 6);
  const maxMsgsInChat = chatsByMsg[0]?.messages.length ?? 1;
  const runtimeReady = diag?.runtimeAvailability.find(r => r.kind === "chat")?.available ?? false;
  const readyRuntimeCount = diag?.runtimeAvailability.filter(runtime => runtime.available).length ?? 0;
  const providerReadyCount = snap.providers.filter(provider => provider.available).length;
  const providerSavedCount = snap.providers.filter(provider => provider.hasKey).length;
  const avgTps      = tpsHistory.length ? tpsHistory.reduce((s,v) => s+v,0) / tpsHistory.length : 0;
  const bestTps     = tpsHistory.length ? Math.max(...tpsHistory) : 0;
  const avgLatency  = latencyHistory.length ? latencyHistory.reduce((s,v) => s+v,0) / latencyHistory.length : 0;
  const accentColor = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() || "#4f8cff";
  const warmColor = "#f0a46b";
  const coolColor = "#79d9ff";
  const greenColor = "#81f7d8";
  const modelStorage = totalModelBytes;
  const otherStorage = Math.max((diag?.diskUsageBytes ?? 0) - modelStorage, 0);
  const healthItems = [
    { label: "Chat runtime", ok: runtimeReady },
    { label: "Model inventory", ok: allModels.length > 0 || providerReadyCount > 0 },
    { label: "Providers", ok: providerReadyCount > 0 || !snap.config.privacy.remoteProviders },
    { label: "Storage", ok: !diag || diag.repoOversizeWarnings.length === 0 },
  ];
  const healthScore = Math.round((healthItems.filter(item => item.ok).length / healthItems.length) * 100);

  return (
    <div className="diag-shell">
      {/* Header row */}
      <div className="diag-header">
        <div className="diag-header-left">
          <h2 className="diag-title">Diagnostics</h2>
          <span className="diag-subtitle">Local system · All data stays on-device</span>
        </div>
        <div className="diag-header-actions">
          <span className={`mode-badge${technicalMode ? " active" : ""}`}>{technicalMode ? "Geek mode" : "Standard"}</span>
          <button className="secondary-button" type="button" disabled={refreshing} onClick={() => void refresh()}>
            <RefreshCw size={13} className={refreshing ? "spin" : ""} /> Refresh
          </button>
        </div>
      </div>

      <div className="diag-health">
        <div className="diag-health-score">
          <strong>{healthScore}%</strong>
          <span>readiness</span>
        </div>
        <div className="diag-health-items">
          {healthItems.map(item => <span key={item.label} className={item.ok ? "ready" : "missing"}>{item.label}</span>)}
        </div>
      </div>

      {/* Top KPI row */}
      <div className="diag-kpi-row">
        <div className="diag-kpi-card">
          <DonutChart pct={runtimeReady ? 100 : 0} color={runtimeReady ? accentColor : "var(--danger)"} size={64} label={runtimeReady ? "✓" : "✗"} />
          <div className="diag-kpi-text">
            <strong>Runtime</strong>
            <span className={runtimeReady ? "diag-ok" : "diag-err"}>{runtimeReady ? "Ready" : "Missing"}</span>
          </div>
        </div>
        <div className="diag-kpi-card">
          <DonutChart pct={allModels.length > 0 ? 100 : 0} color={allModels.length > 0 ? accentColor : "var(--faint)"} size={64} label={String(allModels.length)} />
          <div className="diag-kpi-text">
            <strong>Models</strong>
            <span>{fmtBytes(totalModelBytes)}</span>
          </div>
        </div>
        <div className="diag-kpi-card">
          <DonutChart pct={Math.min((totalChats / 10) * 100, 100)} color={accentColor} size={64} label={String(totalChats)} />
          <div className="diag-kpi-text">
            <strong>Chats</strong>
            <span>{totalMsgs} messages</span>
          </div>
        </div>
        <div className="diag-kpi-card">
          <DonutChart pct={sysMeta.ramGb ? Math.min((totalModelBytes / (sysMeta.ramGb * 1024 * 1024 * 1024)) * 100, 100) : 0} color={accentColor} size={64} label={sysMeta.ramGb ? `${sysMeta.ramGb}G` : "?"} />
          <div className="diag-kpi-text">
            <strong>RAM hint</strong>
            <span>{sysMeta.cores ? `${sysMeta.cores} cores` : "unknown CPU"}</span>
          </div>
        </div>
        <div className="diag-kpi-card">
          <DonutChart pct={diag ? Math.min((diag.diskUsageBytes / (256 * 1024 * 1024)) * 100, 100) : 0} color={accentColor} size={64} label={fmtBytes(diag?.diskUsageBytes ?? 0).split(" ")[0]} />
          <div className="diag-kpi-text">
            <strong>Disk usage</strong>
            <span>{fmtBytes(diag?.diskUsageBytes ?? 0)}</span>
          </div>
        </div>
      </div>

      <div className="diag-grid">

        {/* Performance over time */}
        <div className="diag-card diag-card-wide">
          <div className="diag-card-head">
            <strong>Generation performance</strong>
            <span>{tpsHistory.length ? `${tpsHistory.length} sample${tpsHistory.length !== 1 ? "s" : ""}` : "No runs yet"}</span>
          </div>
          {tpsHistory.length >= 2 ? (
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 12 }}>
              <div className="diag-sparkcard">
                <div className="diag-spark-label">Tokens / sec</div>
                <div className="diag-spark-value">{avgTps.toFixed(1)}<small> avg</small></div>
                <SparkLine values={tpsHistory} color={accentColor} height={44} />
                <div className="diag-spark-sub">Best: {bestTps.toFixed(1)} tok/s</div>
              </div>
              <div className="diag-sparkcard">
                <div className="diag-spark-label">Latency (s)</div>
                <div className="diag-spark-value">{avgLatency.toFixed(1)}<small> avg</small></div>
                <SparkLine values={latencyHistory} color="#f0a46b" height={44} />
                <div className="diag-spark-sub">Last: {latencyHistory[latencyHistory.length-1]?.toFixed(1)}s</div>
              </div>
              <div className="diag-sparkcard">
                <div className="diag-spark-label">Output tokens</div>
                <div className="diag-spark-value">{tokenHistory[tokenHistory.length-1] ?? 0}<small> last</small></div>
                <SparkLine values={tokenHistory} color="#81f7d8" height={44} />
                <div className="diag-spark-sub">Total: {tokenHistory.reduce((s,v)=>s+v,0).toLocaleString()}</div>
              </div>
            </div>
          ) : (
            <div className="diag-empty">Send a message to collect performance data.<br/>Tokens/sec, latency, and output size will chart here.</div>
          )}
          {lastStats && (
            <div className="diag-perf-row">
              <div><span>Last output</span><strong>{lastStats.outputTokens} tok</strong></div>
              <div><span>Speed</span><strong>{lastStats.tps.toFixed(1)} tok/s</strong></div>
              <div><span>Time</span><strong>{(lastStats.elapsedMs/1000).toFixed(2)}s</strong></div>
              <div><span>Est. total ctx</span><strong>~{estimateTokens(snap.chats.find(c=>c.id===lastStats.chatId)?.messages.map(m=>m.text).join(" ") ?? "")} tok</strong></div>
            </div>
          )}
        </div>

        <div className="diag-card diag-card-wide">
          <div className="diag-card-head"><strong>Resource mix</strong><span>models, storage, messages, providers</span></div>
          <div className="diag-matrix-grid">
            <div className="diag-sparkcard">
              <div className="diag-spark-label">Model storage by type</div>
              <SegmentedBar segments={modelBytesByKind.map((item, index) => ({
                label: `${item.label} ${fmtBytes(item.value)}`,
                value: item.value,
                color: [accentColor, warmColor, coolColor][index],
              }))} />
            </div>
            <div className="diag-sparkcard">
              <div className="diag-spark-label">Model count by type</div>
              <SegmentedBar segments={modelCountByKind.map((item, index) => ({
                label: item.label,
                value: item.value,
                color: [accentColor, warmColor, coolColor][index],
              }))} />
            </div>
            <div className="diag-sparkcard">
              <div className="diag-spark-label">Message mix</div>
              <SegmentedBar segments={roleCounts.map((item, index) => ({
                label: item.label,
                value: item.value,
                color: [accentColor, greenColor, warmColor][index],
              }))} />
            </div>
            <div className="diag-sparkcard">
              <div className="diag-spark-label">Provider readiness</div>
              <div className="diag-readiness-grid">
                <div><span>Ready</span><strong>{providerReadyCount}</strong></div>
                <div><span>Saved keys</span><strong>{providerSavedCount}</strong></div>
                <div><span>Runtimes</span><strong>{readyRuntimeCount}/{diag?.runtimeAvailability.length ?? 0}</strong></div>
              </div>
            </div>
            <div className="diag-sparkcard">
              <div className="diag-spark-label">Data folder composition</div>
              <SegmentedBar segments={[
                { label: `models ${fmtBytes(modelStorage)}`, value: modelStorage, color: accentColor },
                { label: `other ${fmtBytes(otherStorage)}`, value: otherStorage, color: warmColor },
              ]} />
            </div>
            <div className="diag-sparkcard">
              <div className="diag-spark-label">Recent token output</div>
              {tokenHistory.length ? <MiniHistogram values={tokenHistory.slice(-18)} color={greenColor} unit="tok" /> : <div className="diag-empty compact">No generation samples yet.</div>}
            </div>
          </div>
        </div>

        {/* Model inventory */}
        <div className="diag-card">
          <div className="diag-card-head"><strong>Models</strong><span>{allModels.length} detected</span></div>
          {allModels.length === 0 ? (
            <div className="diag-empty">No models detected. Place .gguf files in the Models folder and click Refresh.</div>
          ) : (
            <>
              <BarChart
                bars={allModels.slice(0,6).map(m => ({ label: m.name.length > 22 ? m.name.slice(0,20)+"…" : m.name, value: m.sizeBytes ?? 0, max: Math.max(...allModels.map(x => x.sizeBytes ?? 0), 1) }))}
                unit=""
                color={accentColor}
              />
              <div className="diag-model-list">
                {allModels.map(m => (
                  <div key={m.id} className="diag-model-row">
                    <span className={`diag-model-dot ${m.missing ? "missing" : m.available ? "ready" : "detected"}`} />
                    <span className="diag-model-name">{m.name}</span>
                    <span className="diag-model-size">{fmtBytes(m.sizeBytes ?? 0)}</span>
                    <span className="diag-model-kind">{m.kind}</span>
                    {technicalMode && <code className="diag-model-id">{m.id} · {m.path}</code>}
                  </div>
                ))}
              </div>
            </>
          )}
        </div>

        {/* Chat stats */}
        <div className="diag-card">
          <div className="diag-card-head"><strong>Chat activity</strong><span>{totalChats} chats · {totalMsgs} msgs</span></div>
          <div className="diag-stat-row"><span>Avg messages / chat</span><strong>{avgMsgs}</strong></div>
          <div className="diag-stat-row"><span>Est. total tokens used</span><strong>{totalTokens.toLocaleString()}</strong></div>
          <div className="diag-stat-row"><span>Archived chats</span><strong>{snap.chats.filter(c=>c.archived).length}</strong></div>
          <div className="diag-stat-row"><span>Favourite chats</span><strong>{snap.chats.filter(c=>c.favorite).length}</strong></div>
          {chatsByMsg.length > 0 && (
            <>
              <div className="diag-section-label" style={{ marginTop: 12 }}>Messages per chat</div>
              <BarChart
                bars={chatsByMsg.map(c => ({ label: c.title.length > 18 ? c.title.slice(0,17)+"…" : c.title, value: c.messages.length, max: maxMsgsInChat }))}
                color={accentColor}
              />
            </>
          )}
        </div>

        {/* Runtimes */}
        <div className="diag-card">
          <div className="diag-card-head"><strong>Runtimes</strong></div>
          {diag ? (
            <div style={{ display: "grid", gap: 8 }}>
              {diag.runtimeAvailability.map(r => (
                <div key={r.kind} className="diag-runtime-row">
                  <div className="diag-runtime-info">
                    <span className={`diag-model-dot ${r.available ? "ready" : "missing"}`} />
                    <span className="diag-runtime-kind">{r.kind}</span>
                    <strong className={r.available ? "diag-ok" : "diag-err"}>{r.available ? "Ready" : "Not found"}</strong>
                  </div>
                  {r.note && <span className="diag-runtime-note">{r.note}</span>}
                  {technicalMode && <code className="diag-runtime-note">{r.path}</code>}
                </div>
              ))}
            </div>
          ) : <div className="diag-empty">Click Refresh to load runtime info.</div>}
        </div>

        {/* System info */}
        <div className="diag-card">
          <div className="diag-card-head"><strong>System</strong></div>
          <div className="diag-stat-row"><span>CPU logical cores</span><strong>{sysMeta.cores || "Unknown"}</strong></div>
          <div className="diag-stat-row"><span>RAM hint</span><strong>{sysMeta.ramGb ? `${sysMeta.ramGb} GB` : "Unknown"}</strong></div>
          <div className="diag-stat-row"><span>Platform</span><strong>{navigator.platform || "Unknown"}</strong></div>
          <div className="diag-stat-row"><span>Runtime</span><strong>Tauri desktop</strong></div>
          {diag && (
            <>
              <div className="diag-section-label" style={{ marginTop: 12 }}>Paths</div>
              <div className="diag-stat-row"><span>App root</span><strong style={{ wordBreak:"break-all",fontSize:"0.68rem" }}>{diag.appRoot}</strong></div>
              <div className="diag-stat-row"><span>Data dir</span><strong style={{ wordBreak:"break-all",fontSize:"0.68rem" }}>{diag.dataDir}</strong></div>
              <div className="diag-stat-row"><span>Runtimes dir</span><strong style={{ wordBreak:"break-all",fontSize:"0.68rem" }}>{diag.runtimesDir}</strong></div>
            </>
          )}
        </div>

        {technicalMode && diag && (
          <div className="diag-card diag-card-wide">
            <div className="diag-card-head"><strong>Geek details</strong><span>IDs, paths, providers</span></div>
            <div className="tech-grid">
              <div>
                <div className="diag-section-label">Provider status</div>
                {snap.providers.map(provider => (
                  <div key={provider.id} className="diag-stat-row">
                    <span>{provider.id}</span>
                    <strong>{provider.available ? "ready" : provider.hasKey ? "key saved" : "empty"} · {provider.defaultModelId}</strong>
                  </div>
                ))}
              </div>
              <div>
                <div className="diag-section-label">Missing model paths</div>
                {diag.missingModelPaths.length
                  ? diag.missingModelPaths.map(path => <code key={path} className="tech-code-line">{path}</code>)
                  : <div className="diag-empty compact">No missing model paths.</div>}
              </div>
              <div>
                <div className="diag-section-label">Directory map</div>
                {Object.entries(snap.directories).slice(0, 12).map(([key, value]) => <code key={key} className="tech-code-line">{key}: {value}</code>)}
              </div>
              <div>
                <div className="diag-section-label">Raw summary</div>
                <pre className="tech-json">{JSON.stringify({
                  appRoot: diag.appRoot,
                  dataDir: diag.dataDir,
                  runtimesDir: diag.runtimesDir,
                  diskUsageBytes: diag.diskUsageBytes,
                  models: diag.detectedModels.length,
                  missing: diag.missingModelPaths.length,
                  warnings: diag.repoOversizeWarnings.length,
                }, null, 2)}</pre>
              </div>
            </div>
          </div>
        )}

        {/* Warnings */}
        {diag && diag.repoOversizeWarnings.length > 0 && (
          <div className="diag-card diag-card-wide">
            <div className="diag-card-head"><strong>⚠ Oversize file warnings</strong><span>{diag.repoOversizeWarnings.length}</span></div>
            <div className="warning-list">{diag.repoOversizeWarnings.map(w => <code key={w}>{w}</code>)}</div>
          </div>
        )}
      </div>
    </div>
  );
}

// ── App ──────────────────────────────────────────────────────────────────────────
export default function App() {
  const [snap, setSnap]   = useState<AppSnapshot | null>(null);
  const [diag, setDiag]   = useState<DiagnosticsReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy]   = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [view, setView]   = useState<ViewId>("chat");
  const [generating, setGenerating] = useState<Record<string, boolean>>({});
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; chatId: string } | null>(null);
  const [dialog, setDialog] = useState<{ type: "delete-chat" | "rename-chat" | "edit-msg"; chatId: string; targetId?: string; title: string; value: string } | null>(null);
  const [customizeForm, setCustomizeForm] = useState<{
    chatId: string; title: string; modelId: string; imageModelId: string; videoModelId: string;
    userName: string; aiName: string; aiInstructions: string;
    userAvatarBase64: string | null; aiAvatarBase64: string | null;
    userAvatarPath: string | null; aiAvatarPath: string | null;
  } | null>(null);

  // auth
  const [setupForm, setSetupForm] = useState({ username: "", password: "", confirm: "" });
  const [lockUser, setLockUser]   = useState("");
  const [lockPass, setLockPass]   = useState("");

  // chat
  const [text, setText]           = useState("");
  const [chatSearch, setChatSearch] = useState("");
  const [incognito, setIncognito] = useState(false);
  const [showGen, setShowGen]     = useState(false);
  const [genPrompt, setGenPrompt] = useState("");
  const [showDrawer, setShowDrawer] = useState(false);
  const [showNewChat, setShowNewChat] = useState(false);
  const [newChatForm, setNewChatForm] = useState(defaultNewForm("New chat"));
  const [newChatTargetId, setNewChatTargetId] = useState<string | null>(null);
  const [chatDraft, setChatDraft] = useState({ title: "", userName: "", aiName: "", modelId: "", imageModelId: "", videoModelId: "", aiInstructions: "" });
  const [lastStats, setLastStats] = useState<GenerationStats | null>(null);
  const [generationHistory, setGenerationHistory] = useState<GenerationStats[]>([]);
  const [incognitoMessages, setIncognitoMessages] = useState<Record<string, ChatMessage[]>>({});

  // settings
  const [pathsDraft, setPathsDraft] = useState<PathConfig | null>(null);
  const [autoLock, setAutoLock]   = useState(15);
  const [threads, setThreads]     = useState<number | "">("");
  const [gpuLayers, setGpuLayers] = useState<number | "">("");
  const [maxTokens, setMaxTokens] = useState<number | "">("");
  const [remoteProvidersEnabled, setRemoteProvidersEnabled] = useState(false);
  const [providerDraft, setProviderDraft] = useState<ProviderDraft>(EMPTY_PROVIDER_DRAFT);
  const [providerTouched, setProviderTouched] = useState<ProviderTouched>(CLEAN_PROVIDER_TOUCHED);
  const [providerVisibility, setProviderVisibility] = useState<ProviderVisibility>(HIDDEN_PROVIDER_KEYS);
  const [themeId, setThemeId]     = useState(DEFAULT_APPEARANCE.themeId);
  const [smooth, setSmooth]       = useState(DEFAULT_APPEARANCE.smooth);
  const [compact, setCompact]     = useState(DEFAULT_APPEARANCE.compact);
  const [autoScroll, setAutoScroll] = useState(DEFAULT_APPEARANCE.autoScroll);
  const [typewriter, setTypewriter] = useState(DEFAULT_APPEARANCE.typewriter);
  const [showPerformance, setShowPerformance] = useState(DEFAULT_APPEARANCE.showPerformance);
  const [wideMessages, setWideMessages] = useState(DEFAULT_APPEARANCE.wideMessages);
  const [technicalMode, setTechnicalMode] = useState(DEFAULT_APPEARANCE.technicalMode);
  const [scale, setScale]         = useState(DEFAULT_APPEARANCE.scale);
  const [msgStyle, setMsgStyle]   = useState<AppearanceSettings["messageStyle"]>(DEFAULT_APPEARANCE.messageStyle);
  const [collapsed, setCollapsed] = useState(DEFAULT_APPEARANCE.sidebarCollapsed);
  const [themeGroup, setThemeGroup] = useState<ThemeGroup>(DEFAULT_APPEARANCE.themeGroup);
  const [profileDraft, setProfileDraft] = useState({ displayName: "", avatarBase64: null as string | null });
  const [characterDraft, setCharacterDraft] = useState({ name: "", description: "", styleNotes: "", avatarBase64: null as string | null });
  const [backups, setBackups] = useState<BackupRecord[]>([]);
  const [backupsLoading, setBackupsLoading] = useState(false);

  const lockRef    = useRef<ReturnType<typeof setTimeout> | null>(null);
  const endRef     = useRef<HTMLDivElement>(null);
  const messageListRef = useRef<HTMLDivElement>(null);
  const taRef      = useRef<HTMLTextAreaElement>(null);
  const autoSelRef = useRef<Record<string, string>>({});
  const pendingTextRef = useRef<Record<string, string>>({});
  const pendingMediaRef = useRef<Record<string, string>>({});
  const theme      = THEMES.find(t => t.id === themeId) ?? THEMES[0];
  const appearanceThemes = themeGroup === "all" ? THEMES : THEMES.filter(t => t.mode === themeGroup);
  const sysMeta    = useMemo(() => ({ cores: (navigator as Navigator & { hardwareConcurrency?: number }).hardwareConcurrency ?? 0, ramGb: (navigator as Navigator & { deviceMemory?: number }).deviceMemory ?? 0 }), []);

  function syncDraftsFromSnapshot(s: AppSnapshot) {
    setPathsDraft(s.config.paths);
    setAutoLock(s.config.security.autoLockMinutes);
    setThreads(s.config.generation?.threads ?? "");
    setGpuLayers(s.config.generation?.gpuLayers ?? "");
    setMaxTokens(s.config.generation?.maxTokens ?? "");
    setThemeId(s.config.appearance.themeId);
    setThemeGroup(s.config.appearance.themeGroup);
    setSmooth(s.config.appearance.smooth);
    setCompact(s.config.appearance.compact);
    setAutoScroll(s.config.appearance.autoScroll);
    setTypewriter(s.config.appearance.typewriter);
    setShowPerformance(s.config.appearance.showPerformance);
    setWideMessages(s.config.appearance.wideMessages);
    setTechnicalMode(s.config.appearance.technicalMode);
    setScale(s.config.appearance.scale);
    setMsgStyle(s.config.appearance.messageStyle);
    setCollapsed(s.config.appearance.sidebarCollapsed);
    setRemoteProvidersEnabled(s.config.privacy.remoteProviders);
    setProfileDraft({ displayName: s.profile?.displayName ?? "", avatarBase64: null });
    setCharacterDraft({
      name: s.character?.name ?? "",
      description: s.character?.description ?? "",
      styleNotes: s.character?.styleNotes ?? "",
      avatarBase64: null,
    });
  }

  // ── Theme ─────────────────────────────────────────────────────────────────────
  useEffect(() => {
    const r = document.documentElement;
    r.dataset.mode     = theme.mode;
    r.dataset.theme    = theme.id;
    r.dataset.msgstyle = msgStyle;
    r.dataset.smooth   = smooth ? "1" : "0";
    r.dataset.compact  = compact ? "1" : "0";
    r.dataset.wide     = wideMessages ? "1" : "0";
    THEME_VAR_NAMES.forEach(name => r.style.removeProperty(name));
    r.style.setProperty("--accent", theme.accent);
    Object.entries(theme.vars ?? {}).forEach(([name, value]) => r.style.setProperty(name, value));
    r.style.setProperty("--font-scale", `${scale}%`);
  }, [theme.id, theme.mode, theme.accent, theme.vars, smooth, compact, wideMessages, scale, msgStyle]);

  const activeChat = useMemo(() => {
    if (!snap) return null;
    return snap.chats.find(c => c.id === snap.activeChatId) ?? snap.chats[0] ?? null;
  }, [snap]);
  const activeMessages = useMemo(() => {
    if (!activeChat) return [];
    return incognito ? [...activeChat.messages, ...(incognitoMessages[activeChat.id] ?? [])] : activeChat.messages;
  }, [activeChat, incognito, incognitoMessages]);

  const scrollToBottom = useCallback((behavior: ScrollBehavior = smooth ? "smooth" : "auto") => {
    if (!autoScroll) return;
    requestAnimationFrame(() => {
      endRef.current?.scrollIntoView({ behavior, block: "end" });
    });
  }, [autoScroll, smooth]);

  // ── Scroll ────────────────────────────────────────────────────────────────────
  useEffect(() => { scrollToBottom("auto"); }, [activeChat?.id, scrollToBottom]);
  useEffect(() => { scrollToBottom(); }, [activeMessages, scrollToBottom]);

  // ── Textarea resize ───────────────────────────────────────────────────────────
  useEffect(() => {
    const el = taRef.current; if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  }, [text]);

  // ── Toast auto-dismiss ────────────────────────────────────────────────────────
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 5000);
    return () => clearTimeout(t);
  }, [toast]);

  // ── Context menu ──────────────────────────────────────────────────────────────
  useEffect(() => {
    const close = (e: MouseEvent) => {
      if (e.target instanceof HTMLElement && e.target.closest(".custom-context-menu")) return;
      setContextMenu(null);
    };
    const onCtx = (e: MouseEvent) => {
      if (!(e.target instanceof HTMLElement)) { setContextMenu(null); return; }
      if (e.target.closest(".custom-context-menu")) { e.preventDefault(); e.stopPropagation(); return; }
      const target = e.target.closest<HTMLElement>("[data-chat-ctx]");
      if (!target?.dataset.chatCtx) { setContextMenu(null); return; }
      e.preventDefault(); e.stopPropagation();
      setContextMenu({ x: e.clientX, y: e.clientY, chatId: target.dataset.chatCtx });
    };
    window.addEventListener("click", close);
    window.addEventListener("contextmenu", onCtx, true);
    return () => { window.removeEventListener("click", close); window.removeEventListener("contextmenu", onCtx, true); };
  }, []);

  // ── Boot ──────────────────────────────────────────────────────────────────────
  useEffect(() => {
    backend.getAppState()
      .then(s => { setSnap(s); syncDraftsFromSnapshot(s); })
      .catch(e => setToast(err(e, "Failed to load")))
      .finally(() => setLoading(false));
  }, []);

  // ── Tauri streaming partial events ───────────────────────────────────────────
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void listen<GenerationPartialEvent>("chat-generation-partial", ev => {
      const { chatId, text: partial } = ev.payload;
      if (!chatId || !partial.trim()) return;
      pendingTextRef.current[`chat-${chatId}`] = partial;
      setSnap(prev => prev ? ({ ...prev, chats: prev.chats.map(c => {
        if (c.id !== chatId) return c;
        return {
          ...c,
          messages: c.messages.map(m => {
            if (!m.id.startsWith("pending-assistant")) return m;
            pendingTextRef.current[m.id] = partial;
            return { ...m, text: partial };
          }),
        };
      }) }) : prev);
      setIncognitoMessages(prev => ({
        ...prev,
        [chatId]: (prev[chatId] ?? []).map(m => {
          if (!m.id.startsWith("pending-assistant")) return m;
          pendingTextRef.current[m.id] = partial;
          return { ...m, text: partial };
        }),
      }));
      scrollToBottom("auto");
    }).then(fn => { if (disposed) fn(); else unlisten = fn; }).catch(() => {/*optional*/});
    return () => { disposed = true; unlisten?.(); };
  }, [scrollToBottom]);

  // ── Auto-lock ─────────────────────────────────────────────────────────────────
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void listen<MediaGenerationProgressEvent>("media-generation-progress", ev => {
      const { chatId, kind, provider, status, progress } = ev.payload;
      const pendingId = pendingMediaRef.current[`${chatId}:${kind}`];
      if (!pendingId) return;
      const percent = typeof progress === "number" ? ` ${Math.round(progress)}%` : "";
      const nextText = `${provider} is generating ${kind}${percent}.\n\nStatus: ${status}`;
      setSnap(prev => prev ? ({
        ...prev,
        chats: prev.chats.map(c => c.id === chatId
          ? {
              ...c,
              messages: c.messages.map(m => m.id === pendingId
                ? {
                    ...m,
                    text: nextText,
                    attachments: m.attachments.map(a => ({ ...a, sourceLabel: `${kind === "video" ? "Video" : "Image"} generating${percent}` })),
                  }
                : m),
            }
          : c),
      }) : prev);
      scrollToBottom("auto");
    }).then(fn => { if (disposed) fn(); else unlisten = fn; }).catch(() => {/*optional*/});
    return () => { disposed = true; unlisten?.(); };
  }, [scrollToBottom]);

  const unlocked = Boolean(snap && !snap.locked);
  useEffect(() => {
    if (!unlocked) return;
    const arm = () => { if (lockRef.current) clearTimeout(lockRef.current); lockRef.current = setTimeout(() => backend.lock().then(setSnap).catch(() => {}), Math.max(autoLock, 1) * 60_000); };
    const evts = ["mousemove","keydown","pointerdown","touchstart"];
    evts.forEach(e => window.addEventListener(e, arm, { passive: true }));
    arm();
    return () => { evts.forEach(e => window.removeEventListener(e, arm)); if (lockRef.current) clearTimeout(lockRef.current); };
  }, [unlocked, autoLock]);

  // ── Derived ───────────────────────────────────────────────────────────────────
  const ctxChat     = useMemo(() => { if (!contextMenu || !snap) return null; return snap.chats.find(c => c.id === contextMenu.chatId) ?? null; }, [contextMenu, snap]);
  const chatModels  = useMemo(() => snap?.chatModels.filter(m => !m.missing)  ?? [], [snap]);
  const imageModels = useMemo(() => snap?.imageModels.filter(m => !m.missing) ?? [], [snap]);
  const videoModels = useMemo(() => snap?.videoModels.filter(m => !m.missing) ?? [], [snap]);
  const providerStatusById = useMemo(
    () => new Map<ProviderStatus["id"], ProviderStatus>((snap?.providers ?? []).map(provider => [provider.id, provider])),
    [snap]
  );
  const readyProviderCount = snap?.providers.filter(provider => provider.available).length ?? 0;
  const deferredSearch = useDeferredValue(chatSearch);
  const visibleChats   = useMemo(() => {
    if (!snap) return [];
    const q = deferredSearch.trim().toLowerCase();
    return snap.chats.filter(c => !c.archived && (!q || c.title.toLowerCase().includes(q) || c.messages.some(m => m.text.toLowerCase().includes(q))));
  }, [snap, deferredSearch]);
  const gallery = useMemo(() => snap?.chats.flatMap(c => c.messages.flatMap(m => m.attachments.map(a => ({ ...a, chatTitle: c.title })))) ?? [], [snap]);
  const modelId     = activeChat?.settings.modelId ?? "";
  const hasModels   = chatModels.length > 0;
  const preferredChatModel = useMemo(() => pickPreferredChatModel(chatModels), [chatModels]);
  const activeModel = chatModels.find(m => m.id === modelId) ?? null;
  const activeProviderUnavailable = Boolean(modelId && !activeModel && modelId.startsWith("provider:"));
  const activeModelLabel = activeModel
    ? (activeModel.path.startsWith("provider://")
      ? activeModel.name
      : `${activeModel.name} · ${fmtBytes(activeModel.sizeBytes)}`)
    : modelId
      ? (activeProviderUnavailable
        ? "Provider key missing or remote providers disabled"
        : "Selected model unavailable")
      : hasModels
        ? "Select a model"
        : "No model";
  const chatRuntime = snap?.runtimes.find(r => r.kind === "chat") ?? null;
  const imageRuntime = snap?.runtimes.find(r => r.kind === "image") ?? null;
  const videoRuntime = snap?.runtimes.find(r => r.kind === "video") ?? null;
  const canGenerateLocalImage = Boolean(imageRuntime?.available && imageModels.length);
  const canGenerateProviderImage = Boolean(providerStatusById.get("openai")?.available);
  const canGenerateLocalVideo = Boolean(videoRuntime?.available && videoModels.length);
  const canGenerateProviderVideo = Boolean(providerStatusById.get("openai")?.available);
  const canGenerateImage = canGenerateLocalImage || canGenerateProviderImage;
  const canGenerateVideo = canGenerateLocalVideo || canGenerateProviderVideo;
  const inputTok    = estimateTokens(text);
  const menuW = 190, menuH = 240;
  const menuX = contextMenu ? clampMenu(contextMenu.x, menuW, window.innerWidth)  : 0;
  const menuY = contextMenu ? clampMenu(contextMenu.y, menuH, window.innerHeight) : 0;

  // ── Auto-select model for active chat ─────────────────────────────────────────
  useEffect(() => {
    if (!snap || snap.locked || !activeChat || !chatModels.length || busy || generating[activeChat.id]) return;
    const preferred = preferredChatModel;
    if (!preferred) return;
    const current = activeChat.settings.modelId ?? "";
    const lastAuto = autoSelRef.current[activeChat.id];
    const shouldAdoptPreferred = !current || current === lastAuto;
    if (!shouldAdoptPreferred || current === preferred.id) return;
    autoSelRef.current[activeChat.id] = preferred.id;
    void backend.setChatSettings(settingsPayload(activeChat, { modelId: preferred.id })).then(setSnap).catch(e => setToast(err(e)));
  }, [activeChat, busy, chatModels, generating, preferredChatModel, snap]);

  // ── Actions ───────────────────────────────────────────────────────────────────
  function toggleIncognito() {
    if (incognito) setIncognitoMessages({});
    setIncognito(v => !v);
  }

  async function selectChat(chatId: string) {
    if (incognito && activeChat?.id !== chatId) setIncognitoMessages({});
    await run(() => backend.selectChat(chatId));
  }

  async function refreshBackups() {
    setBackupsLoading(true);
    try { setBackups(await backend.listBackups()); }
    catch (e) { setToast(err(e)); }
    finally { setBackupsLoading(false); }
  }

  async function createBackup() {
    if (busy) return;
    setBusy(true);
    try {
      const result = await backend.createBackup();
      setSnap(result.snapshot);
      syncDraftsFromSnapshot(result.snapshot);
      setBackups(prev => [result.backup, ...prev.filter(item => item.id !== result.backup.id)]);
      setToast(`Backup created: ${result.backup.path}`);
    } catch (e) { setToast(err(e)); } finally { setBusy(false); }
  }

  async function run(fn: () => Promise<AppSnapshot>) {
    if (busy) return;
    setBusy(true);
    try { const s = await fn(); setSnap(s); syncDraftsFromSnapshot(s); }
    catch (e) { setToast(err(e)); }
    finally { setBusy(false); }
  }

  async function saveIdentitySettings() {
    if (!snap || busy) return;
    const displayName = profileDraft.displayName.trim();
    const characterName = characterDraft.name.trim();
    if (!displayName || !characterName) {
      setToast("Profile and assistant names are required");
      return;
    }

    setBusy(true);
    try {
      let next = snap;
      if (
        displayName !== (snap.profile?.displayName ?? "") ||
        profileDraft.avatarBase64
      ) {
        next = await backend.saveProfile({
          displayName,
          avatarBase64: profileDraft.avatarBase64,
        });
      }
      if (
        characterName !== (next.character?.name ?? "") ||
        characterDraft.description !== (next.character?.description ?? "") ||
        characterDraft.styleNotes !== (next.character?.styleNotes ?? "") ||
        characterDraft.avatarBase64
      ) {
        next = await backend.saveCharacter({
          name: characterName,
          description: characterDraft.description,
          styleNotes: characterDraft.styleNotes,
          avatarBase64: characterDraft.avatarBase64,
        });
      }
      setSnap(next);
      syncDraftsFromSnapshot(next);
      setToast("Identity saved");
    } catch (e) { setToast(err(e)); } finally { setBusy(false); }
  }

  function openDrawer() {
    if (!activeChat) return;
    setChatDraft({ title: activeChat.title, userName: activeChat.settings.userName ?? "", aiName: activeChat.settings.aiName ?? "", modelId: activeChat.settings.modelId ?? "", imageModelId: activeChat.settings.imageModelId ?? "", videoModelId: activeChat.settings.videoModelId ?? "", aiInstructions: activeChat.settings.aiInstructions ?? "" });
    setShowDrawer(true);
  }

  function openCustomize(c: AppSnapshot["chats"][number]) {
    setCustomizeForm({
      chatId: c.id, title: c.title,
      modelId: c.settings.modelId ?? "", imageModelId: c.settings.imageModelId ?? "", videoModelId: c.settings.videoModelId ?? "",
      userName: c.settings.userName ?? snap?.profile?.displayName ?? "",
      aiName: c.settings.aiName ?? snap?.character?.name ?? "",
      aiInstructions: c.settings.aiInstructions ?? "",
      userAvatarBase64: null, aiAvatarBase64: null,
      userAvatarPath: c.settings.userAvatar ?? snap?.profile?.avatarPath ?? null,
      aiAvatarPath: c.settings.aiAvatar ?? snap?.character?.avatarPath ?? null,
    });
  }

  function openCtxMenu(e: ReactMouseEvent<HTMLElement>, chatId: string) {
    e.preventDefault(); e.stopPropagation();
    setContextMenu({ x: e.clientX, y: e.clientY, chatId });
  }

  function patchPendingAssistant(chatId: string, pendingId: string, nextText: string, incognitoMode: boolean) {
    pendingTextRef.current[pendingId] = nextText;
    if (incognitoMode) {
      setIncognitoMessages(prev => ({
        ...prev,
        [chatId]: (prev[chatId] ?? []).map(m => m.id === pendingId ? { ...m, text: nextText } : m),
      }));
      return;
    }
    setSnap(prev => prev ? ({
      ...prev,
      chats: prev.chats.map(c => c.id === chatId
        ? { ...c, messages: c.messages.map(m => m.id === pendingId ? { ...m, text: nextText } : m) }
        : c),
    }) : prev);
  }

  async function revealAssistantText(chatId: string, pendingId: string, finalText: string, incognitoMode: boolean) {
    if (!typewriter || !finalText.trim()) {
      patchPendingAssistant(chatId, pendingId, finalText, incognitoMode);
      scrollToBottom();
      return;
    }

    const current = pendingTextRef.current[pendingId] ?? "";
    const startsWithPartial = !current.startsWith("Thinking") && finalText.startsWith(current);
    let index = startsWithPartial ? current.length : 0;
    const step = Math.max(6, Math.ceil(finalText.length / 90));

    while (index < finalText.length) {
      index = Math.min(finalText.length, index + step);
      patchPendingAssistant(chatId, pendingId, finalText.slice(0, index), incognitoMode);
      scrollToBottom("auto");
      await sleep(smooth ? 14 : 0);
    }
  }

  async function send() {
    if (!snap || !activeChat || !text.trim() || generating[activeChat.id]) return;
    const msg = text.trim(); const chatId = activeChat.id; const now = new Date().toISOString();
    const pu: ChatMessage = { id: `${incognito ? "incognito-user" : "pending-user"}-${Date.now()}`, role: "user", text: msg, createdAt: now, pinned: false, attachments: [] };
    const pa: ChatMessage = { id: `pending-assistant-${Date.now()}`, role: "assistant", text: "Thinking…", createdAt: now, pinned: false, attachments: [] };
    pendingTextRef.current[pa.id] = pa.text;
    setText("");
    setGenerating(g => ({ ...g, [chatId]: true }));
    const t0 = performance.now();
    if (incognito) {
      setIncognitoMessages(prev => ({ ...prev, [chatId]: [...(prev[chatId] ?? []), pu, pa] }));
    } else {
      setSnap(prev => prev ? ({ ...prev, chats: prev.chats.map(c => c.id === chatId ? { ...c, messages: [...c.messages, pu, pa], updatedAt: now } : c) }) : prev);
    }
    scrollToBottom("auto");
    try {
      let asstMsg: ChatMessage | undefined;
      if (incognito) {
        const assistant = await backend.sendIncognitoChatMessage(chatId, msg);
        asstMsg = assistant;
        await revealAssistantText(chatId, pa.id, assistant.text, true);
        setIncognitoMessages(prev => ({
          ...prev,
          [chatId]: (prev[chatId] ?? []).map(m => m.id === pa.id ? assistant : m),
        }));
      } else {
        const next = await backend.sendChatMessage(chatId, msg);
        const updated = next.chats.find(c => c.id === chatId);
        asstMsg = [...(updated?.messages ?? [])].reverse().find(m => m.role === "assistant");
        await revealAssistantText(chatId, pa.id, asstMsg?.text ?? "", false);
        setSnap(next);
      }
      delete pendingTextRef.current[pa.id];
      const t1 = performance.now();
      const tok = estimateTokens(asstMsg?.text ?? "");
      const ms  = Math.max(1, Math.round(t1 - t0));
      const stats = { chatId, elapsedMs: ms, outputTokens: tok, tps: tok / (ms / 1000) };
      setLastStats(stats);
      setGenerationHistory(history => [...history.slice(-HISTORY_MAX + 1), stats]);
    } catch (e) {
      const msg2 = `Send failed: ${err(e)}`;
      setToast(msg2);
      if (incognito) {
        setIncognitoMessages(prev => ({ ...prev, [chatId]: (prev[chatId] ?? []).map(m => m.id === pa.id ? { ...m, text: msg2 } : m) }));
      } else {
        try {
          const freshSnap = await backend.getAppState();
          setSnap(freshSnap);
        } catch (e2) {
          setToast("Failed to resync state: " + err(e2));
        }
      }
    } finally {
      delete pendingTextRef.current[pa.id];
      setGenerating(g => { const n = { ...g }; delete n[chatId]; return n; });
    }
  }

  function onKey(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); void send(); }
  }

  async function attach(e: ChangeEvent<HTMLInputElement>, kind: MediaAsset["kind"]) {
    if (!activeChat) return;
    const input = e.currentTarget; const file = input.files?.[0]; if (!file) return;
    if (incognito) { setToast("Attachments are disabled in incognito because imported files are saved locally."); input.value = ""; return; }
    if (file.size > MAX_ATTACHMENT_BYTES) { setToast(`Too large: ${fmtBytes(file.size)}`); input.value = ""; return; }
    try { const b64 = await toB64(file); await run(() => backend.importMediaAttachment(activeChat.id, file.name, b64, kind)); }
    catch (error) { setToast(err(error, "Attach failed")); }
    finally { input.value = ""; }
  }

  async function generate(kind: "image" | "video") {
    if (!activeChat || !genPrompt.trim() || busy) return;
    if (incognito) { setToast("Media generation is disabled in incognito because outputs are saved locally."); return; }
    const request = genPrompt.trim();
    const ready = kind === "image" ? canGenerateImage : canGenerateVideo;
    const runtime = kind === "image" ? imageRuntime : videoRuntime;
    const models = kind === "image" ? imageModels : videoModels;
    if (!ready) {
      if (kind === "image") {
        const openaiStatus = providerStatusById.get("openai");
        const providerHint = openaiStatus?.hasKey
          ? "enable remote providers"
          : "add an OpenAI API key";
        setToast(`Add a local image runtime and model, or ${providerHint}, before generating images.`);
        return;
      }
      const openaiStatus = providerStatusById.get("openai");
      const providerHint = openaiStatus?.hasKey
        ? "enable remote providers"
        : "add an OpenAI API key";
      const missing = !runtime?.available || models.length === 0
        ? `Add a local video runtime/model, or ${providerHint}, before generating videos.`
        : `Add ${kind} support before generating ${kind}s.`;
      setToast(missing);
      return;
    }
    const pendingId = `pending-media-${kind}-${Date.now()}`;
    const pendingMessage: ChatMessage = {
      id: pendingId,
      role: "media",
      text: `Generating ${kind}.\n\nPrompt:\n${request}`,
      createdAt: new Date().toISOString(),
      pinned: false,
      attachments: [{
        id: `${pendingId}-asset`,
        kind,
        path: `pending://${kind}/${pendingId}`,
        thumbnailPath: null,
        contentPreview: request,
        prompt: request,
        status: "pending",
        modelUsed: ((canGenerateLocalImage && kind === "image") || (canGenerateLocalVideo && kind === "video")) ? "Local runtime" : "OpenAI provider",
        seed: null,
        sourceLabel: `${kind === "image" ? "Image" : "Video"} generating`,
      }],
    };
    pendingMediaRef.current[`${activeChat.id}:${kind}`] = pendingId;
    setSnap(prev => prev ? ({
      ...prev,
      chats: prev.chats.map(c => c.id === activeChat.id
        ? { ...c, messages: [...c.messages, pendingMessage], updatedAt: pendingMessage.createdAt }
        : c),
    }) : prev);
    scrollToBottom("auto");
    setBusy(true);
    try {
      const fn = kind === "image" ? backend.generateImageFromChat : backend.generateVideoFromChat;
      setSnap(await fn(activeChat.id, request));
      setGenPrompt(""); setShowGen(false);
    } catch (e) {
      const message = `${kind === "image" ? "Image" : "Video"} generation failed: ${err(e)}`;
      setToast(message);
      setSnap(prev => prev ? ({
        ...prev,
        chats: prev.chats.map(c => c.id === activeChat.id
          ? {
              ...c,
              messages: c.messages.map(m => m.id === pendingId
                ? {
                    ...m,
                    text: message,
                    attachments: m.attachments.map(a => ({ ...a, status: "failed", sourceLabel: "Generation failed" })),
                  }
                : m),
            }
          : c),
      }) : prev);
    } finally {
      delete pendingMediaRef.current[`${activeChat.id}:${kind}`];
      setBusy(false);
    }
  }

  async function saveChatDraft() {
    if (!activeChat || busy) return;
    const title = chatDraft.title.trim();
    if (!title) { setToast("Title required"); return; }
    const settingsSame = chatDraft.userName === (activeChat.settings.userName ?? "") && chatDraft.aiName === (activeChat.settings.aiName ?? "") && chatDraft.modelId === (activeChat.settings.modelId ?? "") && chatDraft.imageModelId === (activeChat.settings.imageModelId ?? "") && chatDraft.videoModelId === (activeChat.settings.videoModelId ?? "") && chatDraft.aiInstructions === (activeChat.settings.aiInstructions ?? "");
    const titleSame    = title === activeChat.title;
    if (settingsSame && titleSame) { setToast("No changes"); return; }
    setBusy(true);
    try {
      let next: AppSnapshot | null = null;
      if (!settingsSame) next = await backend.setChatSettings(settingsPayload(activeChat, { modelId: chatDraft.modelId || null, imageModelId: chatDraft.imageModelId || null, videoModelId: chatDraft.videoModelId || null, userName: chatDraft.userName || null, aiName: chatDraft.aiName || null, aiInstructions: chatDraft.aiInstructions || null }));
      if (!titleSame)    next = await backend.renameChat(activeChat.id, title);
      if (next) setSnap(next);
      setToast("Saved");
    } catch (e) { setToast(err(e)); } finally { setBusy(false); }
  }

  async function saveAppSettings() {
    if (!pathsDraft || busy) return;
    setBusy(true);
    try {
      const next = await backend.saveSettings({
        paths: pathsDraft,
        autoLockMinutes: autoLock,
        appearance: {
          themeId,
          themeGroup,
          smooth,
          compact,
          autoScroll,
          typewriter,
          showPerformance,
          wideMessages,
          technicalMode,
          scale,
          messageStyle: msgStyle,
          sidebarCollapsed: collapsed,
        },
        generation: {
          threads: threads === "" ? null : Math.max(1, Number(threads)),
          gpuLayers: gpuLayers === "" ? null : Math.max(0, Number(gpuLayers)),
          maxTokens: maxTokens === "" ? null : Math.max(512, Number(maxTokens)),
        },
        remoteProviders: remoteProvidersEnabled,
        providerKeys: {
          openaiKey: providerTouched.openaiKey ? providerDraft.openaiKey : null,
          openrouterKey: providerTouched.openrouterKey ? providerDraft.openrouterKey : null,
          geminiKey: providerTouched.geminiKey ? providerDraft.geminiKey : null,
          claudeKey: providerTouched.claudeKey ? providerDraft.claudeKey : null,
          mistralKey: providerTouched.mistralKey ? providerDraft.mistralKey : null,
        },
      });
      setSnap(next); syncDraftsFromSnapshot(next);
      setProviderDraft(EMPTY_PROVIDER_DRAFT);
      setProviderTouched(CLEAN_PROVIDER_TOUCHED);
      setProviderVisibility(HIDDEN_PROVIDER_KEYS);
      setToast("Settings saved");
    } catch (e) { setToast(`Not saved: ${err(e)}`); } finally { setBusy(false); }
  }

  async function rescan() {
    setBusy(true);
    try {
      const next = await backend.rescanModels(); setSnap(next);
      if (activeChat && !activeChat.settings.modelId) {
        const first = pickPreferredChatModel(next.chatModels);
        if (first) setSnap(await backend.setChatSettings(settingsPayload(activeChat, { modelId: first.id })));
      }
    } catch (e) { setToast(err(e)); } finally { setBusy(false); }
  }

  // ── Screens ───────────────────────────────────────────────────────────────────
  if (loading) return <div className="screen-center"><div className="loading-spinner" /></div>;

  if (!snap?.setupComplete) {
    const pwOk = setupForm.password.length >= 8 && setupForm.password === setupForm.confirm;
    return (
      <div className="auth-screen">
        <form className="auth-box" onSubmit={async e => {
          e.preventDefault();
          if (!setupForm.username.trim()) { setToast("Username required"); return; }
          if (!pwOk) { setToast("Password must be 8+ chars and match"); return; }
          await run(() => backend.initializeSetup({ username: setupForm.username.trim(), password: setupForm.password, displayName: setupForm.username.trim(), profileAvatarBase64: null, characterName: "Assistant", characterDescription: "Local AI", characterStyleNotes: "Helpful, concise, honest.", characterAvatarBase64: null }));
        }}>
          <div className="auth-logo">AI</div>
          <div className="auth-head"><h1>Set up AI Chat</h1><p>Create an encrypted local account. Nothing leaves this device.</p></div>
          {toast && <div className="auth-error">{toast}</div>}
          <label className="field"><span>Username</span><input required autoFocus autoComplete="username" placeholder="Username" value={setupForm.username} onChange={e => setSetupForm(p => ({ ...p, username: e.target.value }))} /></label>
          <label className="field"><span>Password</span><input required type="password" autoComplete="new-password" placeholder="8+ characters" value={setupForm.password} onChange={e => setSetupForm(p => ({ ...p, password: e.target.value }))} /></label>
          <label className="field"><span>Confirm password</span><input required type="password" autoComplete="new-password" placeholder="Repeat" value={setupForm.confirm} onChange={e => setSetupForm(p => ({ ...p, confirm: e.target.value }))} /></label>
          <div className="password-meter" data-ok={pwOk}><div className="pm-bar" /><span>{setupForm.password.length < 8 ? "Use 8+ characters" : setupForm.password !== setupForm.confirm ? "Passwords don't match" : "✓ Ready"}</span></div>
          <button className="primary-button full" disabled={busy || !pwOk || !setupForm.username.trim()} type="submit"><UserPlus size={15} /> Create account</button>
          <div className="auth-badges"><span>Fully offline</span><span>AES-256</span><span>USB portable</span></div>
        </form>
      </div>
    );
  }

  if (snap.locked) {
    return (
      <div className="lock-shell">
        <div className="lock-hero">
          <div className="lock-eyebrow"><Lock size={13} /> Encrypted · Offline · Local</div>
          <h1>AI Chat</h1>
          <p>Your private local AI workspace. All models, chats, and history stay on this device — no cloud, no telemetry.</p>
          <div className="lock-features"><span>GGUF / llama.cpp</span><span>AES-256</span><span>USB portable</span><span>Air-gapped</span></div>
        </div>
        <div className="lock-card">
          <div className="lock-card-head"><div className="lock-icon"><Lock size={20} /></div><div><strong>Unlock</strong><span>Sign in to your local account</span></div></div>
          {toast && <div className="auth-error">{toast}</div>}
          <form onSubmit={async e => { e.preventDefault(); if (!lockUser.trim() || !lockPass) { setToast("Username and password required"); return; } await run(() => backend.unlockWithPassword(lockUser.trim(), lockPass)); setLockPass(""); setLockUser(""); }}>
            <label className="field"><span>Username</span><input required autoFocus autoComplete="username" placeholder="Username" value={lockUser} onChange={e => setLockUser(e.target.value)} /></label>
            <label className="field" style={{ marginTop: 8 }}><span>Password</span><input required type="password" autoComplete="current-password" placeholder="Password" value={lockPass} onChange={e => setLockPass(e.target.value)} /></label>
            <button className="primary-button full" style={{ marginTop: 14, borderRadius: 9, minHeight: 42 }} disabled={busy || !lockUser.trim() || !lockPass} type="submit"><Lock size={14} /> Unlock</button>
          </form>
          <p className="lock-footnote">Data is encrypted locally. No recovery if you forget your password.</p>
        </div>
      </div>
    );
  }

  // ── Main app ───────────────────────────────────────────────────────────────────
  return (
    <div className={`app-shell${collapsed ? " collapsed" : ""}`}>

      {/* Sidebar */}
      <aside className="sidebar">
        <header className="brand">
          <div className="brand-badge">AI</div>
          {!collapsed && <div><strong>AI Chat</strong><span className="tiny">Local · Offline</span></div>}
          <button className="icon-button" type="button" title={collapsed ? "Expand" : "Collapse"} onClick={() => setCollapsed(v => !v)}>
            {collapsed ? <ChevronRight size={15} /> : <PanelLeftClose size={15} />}
          </button>
        </header>
        <nav className="nav-list">
          {VIEWS.map(v => (
            <button key={v.id} className={`nav-button${view === v.id ? " active" : ""}`} type="button" title={v.label} onClick={() => { setView(v.id); if (v.id === "backups") void refreshBackups(); }}>
              {v.icon}{!collapsed && <span>{v.label}</span>}
            </button>
          ))}
        </nav>
        {!collapsed && view === "chat" && (
          <div className="sidebar-chat-section">
            <div className="sidebar-section-head">
              <label className="search-box"><Search size={13} /><input value={chatSearch} onChange={e => setChatSearch(e.target.value)} placeholder="Search chats…" /></label>
              <button className="icon-button" title="New chat" type="button" onClick={() => { setNewChatTargetId(null); setNewChatForm(defaultNewForm(nextTitle(snap.chats), preferredChatModel?.id ?? "")); setShowNewChat(true); }}><Plus size={15} /></button>
            </div>
            <div className="chat-items">
              {visibleChats.map(c => (
                <button key={c.id} className={`chat-item${c.id === activeChat?.id ? " active" : ""}`} type="button" data-chat-ctx={c.id}
                  onClick={() => void selectChat(c.id)}
                  onContextMenu={e => openCtxMenu(e, c.id)}>
                  <span className="chat-item-title">{c.favorite ? "★ " : ""}{c.title}</span>
                  <span className="chat-item-preview">{chatPreview(c)}</span>
                  <span className="chat-item-date">{fmtShortDate(c.updatedAt)}</span>
                </button>
              ))}
              {!visibleChats.length && <p className="sidebar-empty">No chats</p>}
            </div>
          </div>
        )}
      </aside>

      {/* Workspace */}
      <main className="workspace">
        <header className="topbar">
          <div className="topbar-left" data-chat-ctx={view === "chat" && activeChat ? activeChat.id : undefined} onContextMenu={view === "chat" && activeChat ? e => openCtxMenu(e, activeChat.id) : undefined}>
            <strong className="topbar-title">{view === "chat" && activeChat ? activeChat.title : VIEWS.find(v => v.id === view)?.label}</strong>
            {view === "chat" && <span className="topbar-subtitle">{activeModelLabel}</span>}
            {view === "chat" && chatRuntime && <span className={`runtime-pill ${chatRuntime.available ? "ready" : "missing"}`}>{chatRuntime.available ? "Runtime ready" : "No runtime"}</span>}
            {view === "chat" && incognito && <span className="incognito-badge"><ShieldOff size={11} /> Incognito</span>}
          </div>
          <div className="topbar-right">
            {view === "chat" && activeChat && (<>
              <select className="model-select" value={modelId} disabled={!hasModels} onChange={e => void run(() => backend.setChatSettings(settingsPayload(activeChat, { modelId: e.target.value || null })))}>
                <option value="">{hasModels ? "— select model —" : "No models"}</option>
                {chatModels.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
              </select>
              <button className="icon-button" title="Rescan models" type="button" disabled={busy} onClick={() => void rescan()}><RefreshCw size={14} className={busy ? "spin" : ""} /></button>
              <button className={`icon-button${incognito ? " active-icon" : ""}`} title="Incognito" type="button" onClick={toggleIncognito}><ShieldOff size={14} /></button>
              <button className={`icon-button${showDrawer ? " active-icon" : ""}`} title="Chat settings" type="button" onClick={() => showDrawer ? setShowDrawer(false) : openDrawer()}><Sliders size={14} /></button>
            </>)}
            <button className="icon-button" title="Lock" type="button" onClick={() => { setIncognito(false); setIncognitoMessages({}); void run(() => backend.lock()); }}><Lock size={14} /></button>
          </div>
        </header>

        <section className="workspace-body">

          {/* ── Chat view ── */}
          {view === "chat" && (
            <div className="chat-layout">
              {!activeChat ? (
                <div className="empty-state"><MessageSquare size={26} className="empty-icon" /><p>No chat open</p><span>Press + to start</span></div>
              ) : (
                <>
                  <div ref={messageListRef} className="message-list" data-chat-ctx={activeChat.id} onContextMenu={e => { if (e.target instanceof HTMLElement && e.target.closest("button,input,textarea,select,a")) return; openCtxMenu(e, activeChat.id); }}>
                    {activeMessages.length === 0 ? (
                      <div className="empty-state">
                        <MessageSquare size={24} className="empty-icon" />
                        <p>New conversation</p>
                        <span>{!hasModels ? "Add a local .gguf model or enable provider chat in Settings, then click ↻" : !modelId ? "Select a model in the toolbar" : activeProviderUnavailable ? "The saved provider key is missing or remote providers are disabled" : incognito ? "Incognito mode — messages work normally, won't be referenced later" : "Type a message below or pick a starter"}</span>
                        <div className="prompt-suggestions">{STARTER_PROMPTS.map(p => <button key={p} type="button" disabled={busy} onClick={() => setText(p)}>{p}</button>)}</div>
                      </div>
                    ) : activeMessages.map(m => {
                      const transient = incognito && (m.id.startsWith("incognito-") || m.id.startsWith("pending-"));
                      return (
                      <article key={m.id} className={`message ${m.role}${m.id.startsWith("pending-") ? " pending" : ""}`} data-chat-ctx={activeChat.id}>
                        <div className="avatar">
                          {m.role === "user" ? (() => { const a = activeChat.settings.userAvatar ?? snap.profile?.avatarPath; return a ? <img alt="" src={backend.toAssetUrl(a)} /> : initial(activeChat.settings.userName ?? snap.profile?.displayName, "U"); })()
                            : (() => { const a = activeChat.settings.aiAvatar ?? snap.character?.avatarPath; return a ? <img alt="" src={backend.toAssetUrl(a)} /> : initial(activeChat.settings.aiName ?? snap.character?.name, "A"); })()}
                        </div>
                        <div className="msg-body">
                          <div className="msg-meta">
                            <strong>{m.role === "user" ? (activeChat.settings.userName ?? snap.profile?.displayName ?? "User") : m.role === "assistant" ? (activeChat.settings.aiName ?? snap.character?.name ?? "Assistant") : "Media"}</strong>
                            <time>{new Date(m.createdAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</time>
                            {m.pinned && <span style={{ color: "var(--accent)", fontSize: "0.65rem" }}>★ pinned</span>}
                          </div>
                          <MessageRenderer text={m.text} role={m.role} />
                          {m.id.startsWith("pending-assistant") && <span className="thinking-dots" aria-hidden="true"><i /><i /><i /></span>}
                          {m.attachments.length > 0 && <div className="attachment-row">{m.attachments.map(a => <MessageAttachmentPreview key={a.id} asset={a} />)}</div>}
                          {!transient && <div className="msg-actions">
                            <button className="msg-action-btn" type="button" onClick={() => void run(() => backend.togglePinnedMessage(activeChat.id, m.id))}><Pin size={11} />{m.pinned ? "Unpin" : "Pin"}</button>
                            <button className="msg-action-btn" type="button" onClick={() => setDialog({ type: "edit-msg", chatId: activeChat.id, targetId: m.id, title: "Edit message", value: m.text })}>Edit</button>
                            <button className="msg-action-btn danger" type="button" onClick={() => void run(() => backend.deleteMessage(activeChat.id, m.id))}><Trash2 size={11} /></button>
                          </div>}
                        </div>
                      </article>
                    );})}
                    <div ref={endRef} />
                  </div>

                  <div className="composer">
                    {!hasModels && <div className="composer-notice">No model — place .gguf in <code>{snap.directories.modelsDir}</code> or enable provider chat in Settings, then click ↻</div>}
                    {hasModels && !modelId && <div className="composer-notice">Select a model from the toolbar to enable AI responses.</div>}
                    {activeProviderUnavailable && <div className="composer-notice">The selected provider model is unavailable. Re-enable the API key or choose another model.</div>}
                    {showGen && <div className="gen-row"><input autoFocus placeholder="Describe what to generate…" value={genPrompt} onChange={e => setGenPrompt(e.target.value)} /><button className="secondary-button" type="button" title={canGenerateImage ? "Generate image" : "Add an image runtime and image model, or enable OpenAI provider image generation"} disabled={!genPrompt.trim() || busy || !canGenerateImage} onClick={() => void generate("image")}><Image size={13}/> Image</button><button className="secondary-button" type="button" title={canGenerateVideo ? "Generate video" : "Add a video runtime/model, or enable OpenAI Sora provider generation"} disabled={!genPrompt.trim() || busy || !canGenerateVideo} onClick={() => void generate("video")}><Film size={13}/> Video</button><button className="icon-button" type="button" onClick={() => { setShowGen(false); setGenPrompt(""); }}><X size={13}/></button></div>}
                    {showGen && (!canGenerateImage || !canGenerateVideo) && <div className="composer-notice">Image generation can use a local image runtime/model or an enabled OpenAI key. Video generation can use a local video runtime/model or an enabled OpenAI key with Sora access.</div>}
                    <div className="composer-input-row">
                      <textarea ref={taRef} rows={1} placeholder={generating[activeChat.id] ? "Thinking…" : "Message… Enter to send, Shift+Enter for newline"} value={text} disabled={!!generating[activeChat.id]} onChange={e => setText(e.target.value)} onKeyDown={onKey} />
                      <button className="send-button" type="button" disabled={!!generating[activeChat.id] || !text.trim()} onClick={() => void send()}><Send size={15} /></button>
                    </div>
                    <div className="composer-toolbar">
                      <div className="composer-toolbar-left" style={generating[activeChat.id] || incognito ? { pointerEvents: "none", opacity: 0.5 } : undefined}>
                        <label className="toolbar-btn" title="Image"><Image size={14} /><input hidden type="file" accept="image/*" onChange={e => void attach(e, "image")} /></label>
                        <label className="toolbar-btn" title="Video"><Film size={14} /><input hidden type="file" accept="video/*" onChange={e => void attach(e, "video")} /></label>
                        <label className="toolbar-btn" title="File"><Paperclip size={14} /><input hidden type="file" onChange={e => void attach(e, "file")} /></label>
                        <button className={`toolbar-btn${showGen ? " active" : ""}`} type="button" disabled={incognito} onClick={() => setShowGen(v => !v)}><Sparkles size={14} /></button>
                      </div>
                      <div className="composer-toolbar-right">
                        <span className="toolbar-badge">~{inputTok} tok</span>
                        {generating[activeChat.id] && <span className="toolbar-badge">Generating…</span>}
                        {showPerformance && lastStats?.chatId === activeChat.id && !generating[activeChat.id] && <span className="toolbar-badge">{lastStats.outputTokens} tok · {lastStats.tps.toFixed(1)} tok/s · {(lastStats.elapsedMs/1000).toFixed(1)}s</span>}
                        {incognito && <span className="toolbar-badge">Incognito</span>}
                      </div>
                    </div>
                  </div>
                </>
              )}
            </div>
          )}

          {/* ── Models view ── */}
          {view === "models" && (
            <section className="view-stack">
              <Collapsible title="Models" icon={<Wrench size={14} />}>
                <p className="compact-note">Directory: <code style={{ fontSize: "0.68rem", wordBreak: "break-all" }}>{snap.directories.modelsDir}</code></p>
                {![...snap.chatModels, ...snap.imageModels, ...snap.videoModels].length ? <p className="compact-note">No models. Add .gguf files or enable provider chat in Settings, then rescan.</p> : (
                  <div className="table-wrap"><table className="list-table"><thead><tr><th>Name</th><th>Type</th><th>Size</th><th>Status</th></tr></thead><tbody>
                    {[...snap.chatModels, ...snap.imageModels, ...snap.videoModels].map(m => <tr key={m.id}><td>{m.name}{technicalMode && <small className="row-tech">{m.id} · {m.path}</small>}</td><td>{m.kind}</td><td>{fmtBytes(m.sizeBytes)}</td><td><span className={`status-dot ${m.missing ? "missing" : m.available ? "ready" : "detected"}`}>{m.missing ? "Missing" : m.available ? "Ready" : "Detected"}</span></td></tr>)}
                  </tbody></table></div>
                )}
                <button className="secondary-button" style={{ marginTop: 10 }} type="button" disabled={busy} onClick={() => void rescan()}><RefreshCw size={13} /> Rescan</button>
              </Collapsible>
              <Collapsible title="Runtimes" icon={<Cpu size={14} />} open={false}>
                <div className="runtime-grid">{snap.runtimes.map(r => <div key={r.id} className="runtime-row"><span>{r.kind}</span><strong>{r.available ? "Ready" : "Missing"}</strong><small>{r.note}</small>{technicalMode && <code className="runtime-path">{r.path}</code>}</div>)}</div>
              </Collapsible>
            </section>
          )}

          {/* ── Gallery view ── */}
          {view === "gallery" && (
            <section className="view-stack"><Collapsible title="Media" icon={<Image size={14} />}>
              {!gallery.length ? <p className="compact-note">No media yet.</p> : <div className="gallery-grid">{gallery.map(a => <MediaCard key={a.id} asset={a} />)}</div>}
            </Collapsible></section>
          )}

          {/* ── Settings view ── */}
          {view === "settings" && pathsDraft && (
            <section className="view-stack settings-stack">
              <div className="settings-hero">
                <div>
                  <strong>Settings</strong>
                  <span>{remoteProvidersEnabled ? "Remote providers enabled" : "Local-first mode"} · {technicalMode ? "Geek details visible" : "Clean controls"}</span>
                </div>
                <span className={`mode-badge${technicalMode ? " active" : ""}`}>{technicalMode ? "Geek mode on" : "Standard mode"}</span>
              </div>
              <div className="settings-quick-grid">
                <div className="settings-quick-card">
                  <span>Models</span>
                  <strong>{chatModels.length + imageModels.length + videoModels.length}</strong>
                  <small>{snap.directories.modelsDir}</small>
                </div>
                <div className="settings-quick-card">
                  <span>Providers</span>
                  <strong>{readyProviderCount}/{snap.providers.length}</strong>
                  <small>{remoteProvidersEnabled ? "Enabled" : "Disabled"}</small>
                </div>
                <div className="settings-quick-card">
                  <span>Privacy</span>
                  <strong>{snap.config.privacy.localOnly ? "Local" : "Mixed"}</strong>
                  <small>Telemetry {snap.config.privacy.telemetry ? "on" : "off"}</small>
                </div>
                <div className="settings-quick-card">
                  <span>Interface</span>
                  <strong>{scale}%</strong>
                  <small>{theme.name} · {msgStyle}</small>
                </div>
              </div>
              <Collapsible title="Identity" icon={<UserPlus size={14} />}>
                <div className="two-col">
                  <div className="customize-section">
                    <span className="customize-section-label">Profile</span>
                    <div className="customize-row">
                      <label className="customize-avatar-upload">
                        <div className="customize-avatar">
                          {profileDraft.avatarBase64 ? <img src={`data:image/png;base64,${profileDraft.avatarBase64}`} alt="" />
                            : snap.profile?.avatarPath ? <img src={backend.toAssetUrl(snap.profile.avatarPath)} alt="" />
                            : <span>{initial(profileDraft.displayName, "U")}</span>}
                          <div className="customize-avatar-overlay"><Image size={13} /></div>
                        </div>
                        <input hidden type="file" accept="image/*" onChange={async e => { const f = e.target.files?.[0]; if (!f) return; if (f.size > 4*1024*1024) { setToast("Under 4 MB please"); return; } const b64 = await toB64(f); setProfileDraft(p => ({ ...p, avatarBase64: b64 })); }} />
                      </label>
                      <label className="field" style={{ flex: 1 }}><span>Display name</span><input value={profileDraft.displayName} onChange={e => setProfileDraft(p => ({ ...p, displayName: e.target.value }))} /></label>
                    </div>
                  </div>
                  <div className="customize-section">
                    <span className="customize-section-label">Assistant</span>
                    <div className="customize-row">
                      <label className="customize-avatar-upload">
                        <div className="customize-avatar ai">
                          {characterDraft.avatarBase64 ? <img src={`data:image/png;base64,${characterDraft.avatarBase64}`} alt="" />
                            : snap.character?.avatarPath ? <img src={backend.toAssetUrl(snap.character.avatarPath)} alt="" />
                            : <span>{initial(characterDraft.name, "A")}</span>}
                          <div className="customize-avatar-overlay"><Image size={13} /></div>
                        </div>
                        <input hidden type="file" accept="image/*" onChange={async e => { const f = e.target.files?.[0]; if (!f) return; if (f.size > 4*1024*1024) { setToast("Under 4 MB please"); return; } const b64 = await toB64(f); setCharacterDraft(p => ({ ...p, avatarBase64: b64 })); }} />
                      </label>
                      <label className="field" style={{ flex: 1 }}><span>Assistant name</span><input value={characterDraft.name} onChange={e => setCharacterDraft(p => ({ ...p, name: e.target.value }))} /></label>
                    </div>
                    <label className="field"><span>Description</span><input value={characterDraft.description} onChange={e => setCharacterDraft(p => ({ ...p, description: e.target.value }))} /></label>
                    <label className="field"><span>Default style notes</span><textarea rows={3} value={characterDraft.styleNotes} onChange={e => setCharacterDraft(p => ({ ...p, styleNotes: e.target.value }))} /></label>
                  </div>
                </div>
                <button className="secondary-button" type="button" disabled={busy || !profileDraft.displayName.trim() || !characterDraft.name.trim()} onClick={() => void saveIdentitySettings()}>Save identity</button>
              </Collapsible>
              <Collapsible title="Appearance" icon={<Palette size={14} />}>
                <div className="theme-mode-tabs" role="tablist" aria-label="Theme groups">
                  {([
                    { id: "all", label: "All", icon: <Palette size={11} /> },
                    { id: "light", label: "Light", icon: <Sun size={11} /> },
                    { id: "dark", label: "Dark", icon: <Moon size={11} /> },
                    { id: "night", label: "Night", icon: <Star size={11} /> },
                  ] as const).map(group => (
                    <button
                      key={group.id}
                      className={`theme-mode-button${themeGroup === group.id ? " active" : ""}`}
                      type="button"
                      role="tab"
                      aria-selected={themeGroup === group.id}
                      onClick={() => setThemeGroup(group.id)}
                    >
                      {group.icon}
                      <span>{group.label}</span>
                    </button>
                  ))}
                </div>
                <div className="theme-grid">{appearanceThemes.map(t => <button key={t.id} className={`theme-tile${themeId === t.id ? " active" : ""}`} type="button" onClick={() => { setThemeGroup(t.mode); setThemeId(t.id); }}><span className="theme-swatch" style={themeSwatchStyle(t)} /><strong>{t.name}</strong><span>{t.mode === "light" ? <Sun size={10} /> : t.mode === "night" ? <Moon size={10} /> : <Palette size={10} />} {t.mode}</span></button>)}</div>
                <label className="toggle-row"><span>Smooth animations</span><input type="checkbox" checked={smooth} onChange={e => setSmooth(e.target.checked)} /></label>
                <label className="toggle-row"><span>Compact density</span><input type="checkbox" checked={compact} onChange={e => setCompact(e.target.checked)} /></label>
                <label className="toggle-row"><span>Auto-scroll during replies</span><input type="checkbox" checked={autoScroll} onChange={e => setAutoScroll(e.target.checked)} /></label>
                <label className="toggle-row"><span>Animated text reveal</span><input type="checkbox" checked={typewriter} onChange={e => setTypewriter(e.target.checked)} /></label>
                <label className="toggle-row"><span>Show performance badges</span><input type="checkbox" checked={showPerformance} onChange={e => setShowPerformance(e.target.checked)} /></label>
                <label className="toggle-row"><span>Wide message column</span><input type="checkbox" checked={wideMessages} onChange={e => setWideMessages(e.target.checked)} /></label>
                <label className="toggle-row"><span>Start with sidebar collapsed</span><input type="checkbox" checked={collapsed} onChange={e => setCollapsed(e.target.checked)} /></label>
                <label className="field"><span>Message style</span><select value={msgStyle} onChange={e => setMsgStyle(e.target.value as AppearanceSettings["messageStyle"])}><option value="cards">Cards</option><option value="bubbles">Bubbles</option><option value="minimal">Minimal</option></select></label>
                <label className="field"><span>Scale — {scale}%</span><input type="range" min={85} max={115} step={5} value={scale} onChange={e => setScale(Number(e.target.value))} /></label>
              </Collapsible>
              <Collapsible title="Generation & Performance" icon={<Cpu size={14} />} open={false}>
                <div className="two-col">
                  <label className="field"><span>CPU threads</span><input type="number" min={1} max={64} value={threads} onChange={e => setThreads(e.target.value === "" ? "" : Number(e.target.value))} placeholder="Auto" /></label>
                  <label className="field"><span>GPU/NPU layers</span><input type="number" min={0} max={999} value={gpuLayers} onChange={e => setGpuLayers(e.target.value === "" ? "" : Number(e.target.value))} placeholder="Auto" /></label>
                  <label className="field"><span>Max tokens</span><input type="number" min={512} max={8192} value={maxTokens} onChange={e => setMaxTokens(e.target.value === "" ? "" : Number(e.target.value))} placeholder="2048" /></label>
                </div>
                <p className="compact-note">Leave CPU threads blank to use all logical cores. Leave GPU layers blank to auto-offload only when llama detects a usable accelerator. Use 2048+ max tokens for derivations, long explanations, and code.</p>
              </Collapsible>
              <Collapsible title="Geek Mode" icon={<Wrench size={14} />} open={technicalMode}>
                <label className="toggle-row">
                  <span>Show technical parts</span>
                  <input type="checkbox" checked={technicalMode} onChange={e => setTechnicalMode(e.target.checked)} />
                </label>
                <p className="compact-note">Shows model IDs, provider IDs, exact runtime paths, raw diagnostic payloads, and deeper debug notes across Settings, Models, and Diagnostics.</p>
                {technicalMode && (
                  <div className="tech-callout">
                    <code>appRoot: {snap.directories.appRoot}</code>
                    <code>dataDir: {snap.directories.dataDir}</code>
                    <code>runtimesDir: {snap.directories.runtimesDir}</code>
                  </div>
                )}
              </Collapsible>
              <Collapsible title="Paths" icon={<FileUp size={14} />} open={false}>
                <div className="two-col">{Object.entries(pathsDraft)
                  .filter(([k]) => technicalMode || ["appRoot", "dataDir", "modelsDir", "imageModelsDir", "videoModelsDir", "runtimesDir", "backupsDir"].includes(k))
                  .map(([k, v]) => <label key={k} className="field"><span>{k}</span><input value={v} onChange={e => setPathsDraft(p => p ? { ...p, [k]: e.target.value } : p)} /></label>)}</div>
                {!technicalMode && <p className="compact-note">Enable Geek mode to edit every internal directory.</p>}
              </Collapsible>
              <Collapsible title="Security" icon={<Lock size={14} />} open={false}>
                <label className="field"><span>Auto-lock (minutes)</span><input type="number" min={1} max={120} value={autoLock} style={{ maxWidth: 80 }} onChange={e => setAutoLock(Number(e.target.value))} /></label>
              </Collapsible>
              <Collapsible title="Providers" icon={<KeyRound size={14} />}>
                <label className="toggle-row">
                  <span>Enable provider chat</span>
                  <input type="checkbox" checked={remoteProvidersEnabled} onChange={e => setRemoteProvidersEnabled(e.target.checked)} />
                </label>
                <p className="compact-note">Provider keys are stored encrypted on this device. Type a new key to replace the saved key, leave a field blank to keep it, or use the delete button and save to remove it. Saved keys stay inactive until you enable remote providers.</p>
                <div className="provider-grid">
                  {PROVIDER_FIELDS.map(field => {
                    const status = providerStatusById.get(field.providerId);
                    const visible = providerVisibility[field.field];
                    return (
                      <div key={field.field} className="provider-row">
                        <div className="provider-row-head">
                          <div>
                            <strong>{field.label}</strong>
                            <span>{status?.note ?? `Uses ${field.defaultModel} by default.`}</span>
                            {technicalMode && <div className="tech-meta"><code>provider:{field.providerId}</code><code>{field.defaultModel}</code></div>}
                          </div>
                          <span className={`status-dot ${status?.available ? "ready" : status?.hasKey ? "detected" : "missing"}`}>{status?.available ? "Ready" : status?.hasKey ? "Saved" : "Empty"}</span>
                        </div>
                        <div className="provider-row-body">
                          <label className="field provider-key-field">
                            <span>API key</span>
                            <div className="provider-key-input">
                              <input
                                type={visible ? "text" : "password"}
                                value={providerDraft[field.field]}
                                placeholder={status?.hasKey ? "Stored securely" : `${field.label} API key`}
                                autoComplete="off"
                                autoCapitalize="off"
                                autoCorrect="off"
                                spellCheck={false}
                                onChange={e => {
                                  const value = e.target.value;
                                  setProviderDraft(prev => ({ ...prev, [field.field]: value }));
                                  setProviderTouched(prev => ({ ...prev, [field.field]: true }));
                                }}
                              />
                              <button
                                className="icon-button"
                                type="button"
                                title={visible ? `Hide ${field.label} key` : `Show ${field.label} key`}
                                onClick={() => setProviderVisibility(prev => ({ ...prev, [field.field]: !prev[field.field] }))}
                              >
                                {visible ? <EyeOff size={12} /> : <Eye size={12} />}
                              </button>
                            </div>
                          </label>
                          <button
                            className="icon-button danger"
                            type="button"
                            title={`Delete saved ${field.label} key on save`}
                            onClick={() => {
                              setProviderDraft(prev => ({ ...prev, [field.field]: "" }));
                              setProviderTouched(prev => ({ ...prev, [field.field]: true }));
                            }}
                          >
                            <Trash2 size={12} />
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </Collapsible>
              <Collapsible title="Privacy" icon={<ShieldOff size={14} />} open={false}>
                <label className="toggle-row"><span>Local-only (always on)</span><input type="checkbox" checked disabled /></label>
                <label className="toggle-row"><span>Remote providers enabled</span><input type="checkbox" checked={snap.config.privacy.remoteProviders} disabled /></label>
                <label className="toggle-row"><span>Telemetry disabled</span><input type="checkbox" checked={!snap.config.privacy.telemetry} disabled /></label>
              </Collapsible>
              <button className="primary-button" type="button" onClick={() => void saveAppSettings()}>Save settings and keys</button>
            </section>
          )}

          {/* ── Diagnostics view ── */}
          {view === "diagnostics" && (
            <DiagnosticsView
              snap={snap}
              diag={diag}
              setDiag={setDiag}
              lastStats={lastStats}
              generationHistory={generationHistory}
              sysMeta={sysMeta}
              technicalMode={technicalMode}
              setToast={setToast}
            />
          )}

          {/* ── Backups view ── */}
          {view === "backups" && (
            <section className="view-stack">
              <Collapsible title="Backups" icon={<Archive size={14} />}>
                <p className="compact-note">Backups: <code style={{ fontSize: "0.68rem" }}>{snap.directories.backupsDir}</code></p>
                <div className="inline-actions">
                  <button className="secondary-button" type="button" disabled={busy || backupsLoading} onClick={() => void createBackup()}><Archive size={13} /> Create backup</button>
                  <button className="secondary-button" type="button" disabled={backupsLoading} onClick={() => void refreshBackups()}><RefreshCw size={13} className={backupsLoading ? "spin" : ""} /> Refresh</button>
                </div>
                {backupsLoading ? <p className="compact-note">Loading backups…</p> : !backups.length ? <p className="compact-note">No backups yet.</p> : (
                  <div className="table-wrap"><table className="list-table"><thead><tr><th>Created</th><th>Files</th><th>Size</th><th>Path</th></tr></thead><tbody>
                    {backups.map(b => <tr key={b.id}><td>{fmtShortDate(b.createdAt)}</td><td>{b.filesCopied}</td><td>{fmtBytes(b.sizeBytes)}</td><td style={{ wordBreak: "break-all" }}>{b.path}</td></tr>)}
                  </tbody></table></div>
                )}
              </Collapsible>
            </section>
          )}

        </section>
      </main>

      {/* Chat drawer */}
      {showDrawer && activeChat && (<>
        <div className="drawer-backdrop" onClick={() => setShowDrawer(false)} />
        <aside className="drawer">
          <div className="drawer-head"><strong>Chat settings</strong><button className="icon-button" type="button" onClick={() => setShowDrawer(false)}><X size={15} /></button></div>
          <div className="drawer-body">
            <label className="field"><span>Title</span><input value={chatDraft.title} onChange={e => setChatDraft(p => ({ ...p, title: e.target.value }))} /></label>
            <label className="field"><span>User name</span><input value={chatDraft.userName} placeholder="User" onChange={e => setChatDraft(p => ({ ...p, userName: e.target.value }))} /></label>
            <label className="field"><span>AI name</span><input value={chatDraft.aiName} placeholder="Assistant" onChange={e => setChatDraft(p => ({ ...p, aiName: e.target.value }))} /></label>
            <label className="field"><span>Chat model</span>
              <select value={chatDraft.modelId} onChange={e => setChatDraft(p => ({ ...p, modelId: e.target.value }))}>
                <option value="">{chatModels.length ? "— none —" : "No models"}</option>
                {chatModels.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
              </select>
            </label>
            <div className="two-col">
              <label className="field"><span>Image model</span>
                <select value={chatDraft.imageModelId} onChange={e => setChatDraft(p => ({ ...p, imageModelId: e.target.value }))}>
                  <option value="">{imageModels.length ? "None" : "No image models"}</option>
                  {imageModels.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
                </select>
              </label>
              <label className="field"><span>Video model</span>
                <select value={chatDraft.videoModelId} onChange={e => setChatDraft(p => ({ ...p, videoModelId: e.target.value }))}>
                  <option value="">{videoModels.length ? "None" : "No video models"}</option>
                  {videoModels.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
                </select>
              </label>
            </div>
            <label className="field"><span>AI instructions</span><textarea rows={5} placeholder="How should the AI behave?" value={chatDraft.aiInstructions} onChange={e => setChatDraft(p => ({ ...p, aiInstructions: e.target.value }))} /></label>
            <div className="preset-row">{INSTRUCTION_PRESETS.map(p => <button key={p.label} className="mini-button" type="button" onClick={() => setChatDraft(d => ({ ...d, aiInstructions: p.value }))}>{p.label}</button>)}</div>
          </div>
          <div className="drawer-footer">
            <button className="primary-button" type="button" disabled={busy || !chatDraft.title.trim()} onClick={() => void saveChatDraft()}>Save</button>
            <button className={`secondary-button${incognito ? " active-soft" : ""}`} type="button" onClick={toggleIncognito}><ShieldOff size={13} /> {incognito ? "End incognito" : "Incognito"}</button>
            <button className="icon-button" title="Favourite" type="button" onClick={() => void run(() => backend.toggleChatFlag(activeChat.id, "favorite"))}><Star size={14} fill={activeChat.favorite ? "currentColor" : "none"} /></button>
            <button className="icon-button" title="Archive"   type="button" onClick={() => void run(() => backend.toggleChatFlag(activeChat.id, "archived"))}><Archive size={14} /></button>
            <button className="icon-button danger" title="Clear messages" type="button" onClick={() => { if (confirm("Clear all messages?")) void run(() => backend.clearChatMessages(activeChat.id)); setShowDrawer(false); }}><Trash2 size={14} /></button>
          </div>
        </aside>
      </>)}

      {/* New chat dialog */}
      {showNewChat && (
        <div className="dialog-backdrop">
          <form className="dialog-card" onSubmit={async e => {
            e.preventDefault();
            if (!newChatForm.title.trim()) { setToast("Title required"); return; }
            setBusy(true);
            try {
              let next: AppSnapshot;
              if (newChatTargetId && snap) {
                const chat = snap.chats.find(c => c.id === newChatTargetId);
                if (!chat) throw new Error("Chat not found");
                next = await backend.renameChat(chat.id, newChatForm.title.trim());
                const renamed = next.chats.find(c => c.id === chat.id) ?? chat;
                next = await backend.setChatSettings(settingsPayload(renamed, { modelId: newChatForm.modelId || null, userName: newChatForm.userName || null, aiName: newChatForm.aiName || null, aiInstructions: newChatForm.instructions || null }));
              } else {
                const created = await backend.createChat(newChatForm.title.trim());
                const chat = created.chats.find(c => c.id === created.activeChatId) ?? created.chats[0];
                next = chat ? await backend.setChatSettings(settingsPayload(chat, { modelId: newChatForm.modelId || null, userName: newChatForm.userName || null, aiName: newChatForm.aiName || null, aiInstructions: newChatForm.instructions || null })) : created;
              }
              setSnap(next); setShowNewChat(false); setNewChatTargetId(null);
            } catch (e2) { setToast(err(e2)); } finally { setBusy(false); }
          }}>
            <div className="dialog-head"><strong>{newChatTargetId ? "Edit chat" : "New chat"}</strong><button className="icon-button" type="button" onClick={() => { setShowNewChat(false); setNewChatTargetId(null); }}><X size={14} /></button></div>
            <label className="field"><span>Title</span><input autoFocus value={newChatForm.title} onChange={e => setNewChatForm(p => ({ ...p, title: e.target.value }))} /></label>
            <label className="field"><span>Model</span>
              <select value={newChatForm.modelId} onChange={e => setNewChatForm(p => ({ ...p, modelId: e.target.value }))}>
                <option value="">{chatModels.length ? "Auto-select first available" : "No models"}</option>
                {chatModels.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
              </select>
            </label>
            <div className="two-col">
              <label className="field"><span>User name</span><input value={newChatForm.userName} placeholder="User" onChange={e => setNewChatForm(p => ({ ...p, userName: e.target.value }))} /></label>
              <label className="field"><span>AI name</span><input value={newChatForm.aiName} placeholder="Assistant" onChange={e => setNewChatForm(p => ({ ...p, aiName: e.target.value }))} /></label>
            </div>
            <label className="field"><span>Instructions</span><textarea rows={3} placeholder="How should the AI behave?" value={newChatForm.instructions} onChange={e => setNewChatForm(p => ({ ...p, instructions: e.target.value }))} /></label>
            <div className="preset-row">{INSTRUCTION_PRESETS.map(p => <button key={p.label} className="mini-button" type="button" onClick={() => setNewChatForm(d => ({ ...d, instructions: p.value }))}>{p.label}</button>)}</div>
            <div className="dialog-actions">
              <button className="secondary-button" type="button" onClick={() => { setShowNewChat(false); setNewChatTargetId(null); }}>Cancel</button>
              <button className="primary-button" disabled={busy || !newChatForm.title.trim()} type="submit">{newChatTargetId ? "Save" : "Create"}</button>
            </div>
          </form>
        </div>
      )}

      {/* Context menu */}
      {contextMenu && ctxChat && (
        <div className="custom-context-menu" style={{ top: menuY, left: menuX }} onClick={e => e.stopPropagation()}>
          <button className="custom-context-menu-item" type="button" onClick={() => { openCustomize(ctxChat); setContextMenu(null); }}><Sliders size={13} /> Customize</button>
          <button className="custom-context-menu-item" type="button" onClick={() => { setContextMenu(null); setDialog({ type: "rename-chat", chatId: ctxChat.id, title: "Rename chat", value: ctxChat.title }); }}><MessageSquare size={13} /> Rename</button>
          <button className="custom-context-menu-item" type="button" onClick={() => { setContextMenu(null); void run(() => backend.toggleChatFlag(ctxChat.id, "favorite")); }}><Star size={13} fill={ctxChat.favorite ? "currentColor" : "none"} /> {ctxChat.favorite ? "Unfavorite" : "Favorite"}</button>
          <button className="custom-context-menu-item" type="button" onClick={() => { setContextMenu(null); void run(() => backend.toggleChatFlag(ctxChat.id, "archived")); }}><Archive size={13} /> {ctxChat.archived ? "Unarchive" : "Archive"}</button>
          <div className="custom-context-menu-divider" />
          <button className="custom-context-menu-item danger" type="button" onClick={() => { setContextMenu(null); setDialog({ type: "delete-chat", chatId: ctxChat.id, title: "Delete chat", value: ctxChat.title }); }}><Trash2 size={13} /> Delete</button>
        </div>
      )}

      {/* Dialogs */}
      {dialog && (
        <div className="dialog-backdrop" onClick={() => setDialog(null)}>
          <form className="dialog-card" onClick={e => e.stopPropagation()} onSubmit={async e => {
            e.preventDefault(); setBusy(true);
            try {
              let next: AppSnapshot | null = null;
              if (dialog.type === "delete-chat") next = await backend.deleteChat(dialog.chatId);
              else if (dialog.type === "rename-chat" && dialog.value.trim()) next = await backend.renameChat(dialog.chatId, dialog.value.trim());
              else if (dialog.type === "edit-msg" && dialog.targetId && dialog.value.trim()) next = await backend.updateMessage(dialog.chatId, dialog.targetId, dialog.value.trim());
              if (next) setSnap(next); setDialog(null);
            } catch (e2) { setToast(err(e2)); } finally { setBusy(false); }
          }}>
            <div className="dialog-head"><strong>{dialog.title}</strong><button className="icon-button" type="button" onClick={() => setDialog(null)}><X size={13} /></button></div>
            {dialog.type === "delete-chat" ? <p style={{ fontSize: "0.85rem", lineHeight: "1.4" }}>Delete <strong>"{dialog.value}"</strong>? This cannot be undone.</p>
              : dialog.type === "rename-chat" ? <label className="field"><span>Name</span><input required autoFocus value={dialog.value} onChange={e => setDialog({ ...dialog, value: e.target.value })} /></label>
              : <label className="field"><span>Message</span><textarea required autoFocus rows={4} value={dialog.value} onChange={e => setDialog({ ...dialog, value: e.target.value })} /></label>}
            <div className="dialog-actions">
              <button className="secondary-button" type="button" onClick={() => setDialog(null)}>Cancel</button>
              <button className={dialog.type === "delete-chat" ? "primary-button" : "primary-button"} style={dialog.type === "delete-chat" ? { background: "var(--danger)", borderColor: "var(--danger)" } : {}} disabled={busy || (dialog.type !== "delete-chat" && !dialog.value.trim())} type="submit">{dialog.type === "delete-chat" ? "Delete" : "Save"}</button>
            </div>
          </form>
        </div>
      )}

      {/* Customize modal */}
      {customizeForm && (
        <div className="dialog-backdrop" onClick={() => setCustomizeForm(null)}>
          <form className="dialog-card" style={{ width: "min(520px, 100%)" }} onClick={e => e.stopPropagation()} onSubmit={async e => {
            e.preventDefault(); setBusy(true);
            try {
              const chat = snap.chats.find(c => c.id === customizeForm.chatId);
              if (!chat) throw new Error("Chat not found");
              let src = chat; let next: AppSnapshot | null = null;
              const title = customizeForm.title.trim();
              if (title && title !== chat.title) { next = await backend.renameChat(chat.id, title); src = next.chats.find(c => c.id === chat.id) ?? chat; }
              next = await backend.setChatSettings(settingsPayload(src, { modelId: customizeForm.modelId || null, imageModelId: customizeForm.imageModelId || null, videoModelId: customizeForm.videoModelId || null, userName: customizeForm.userName || null, aiName: customizeForm.aiName || null, aiInstructions: customizeForm.aiInstructions || null, userAvatarBase64: customizeForm.userAvatarBase64, aiAvatarBase64: customizeForm.aiAvatarBase64, userAvatarPath: customizeForm.userAvatarPath, aiAvatarPath: customizeForm.aiAvatarPath }));
              setSnap(next); setCustomizeForm(null);
            } catch (e2) { setToast(err(e2)); } finally { setBusy(false); }
          }}>
            <div className="dialog-head"><strong>Customize Chat</strong><button className="icon-button" type="button" onClick={() => setCustomizeForm(null)}><X size={13} /></button></div>

            {/* Title + models */}
            <div className="customize-section">
              <span className="customize-section-label">Chat</span>
              <label className="field"><span>Title</span><input value={customizeForm.title} onChange={e => setCustomizeForm(p => p ? { ...p, title: e.target.value } : p)} /></label>
              <div className="two-col">
                <label className="field"><span>Chat model</span>
                  <select value={customizeForm.modelId} onChange={e => setCustomizeForm(p => p ? { ...p, modelId: e.target.value } : p)}>
                    <option value="">{chatModels.length ? "Auto" : "No models"}</option>
                    {chatModels.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
                  </select>
                </label>
                <label className="field"><span>Image model</span>
                  <select value={customizeForm.imageModelId} onChange={e => setCustomizeForm(p => p ? { ...p, imageModelId: e.target.value } : p)}>
                    <option value="">None</option>{imageModels.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
                  </select>
                </label>
              </div>
              <label className="field"><span>Video model</span>
                <select value={customizeForm.videoModelId} onChange={e => setCustomizeForm(p => p ? { ...p, videoModelId: e.target.value } : p)}>
                  <option value="">{videoModels.length ? "None" : "No video models"}</option>
                  {videoModels.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
                </select>
              </label>
            </div>

            {/* User */}
            <div className="customize-section">
              <span className="customize-section-label">You</span>
              <div className="customize-row">
                <label className="customize-avatar-upload">
                  <div className="customize-avatar">
                    {customizeForm.userAvatarBase64 ? <img src={`data:image/png;base64,${customizeForm.userAvatarBase64}`} alt="" />
                      : customizeForm.userAvatarPath ? <img src={backend.toAssetUrl(customizeForm.userAvatarPath)} alt="" />
                      : <span>{customizeForm.userName[0]?.toUpperCase() || "U"}</span>}
                    <div className="customize-avatar-overlay"><Image size={13} /></div>
                  </div>
                  <input hidden type="file" accept="image/*" onChange={async e => { const f = e.target.files?.[0]; if (!f) return; if (f.size > 4*1024*1024) { setToast("Under 4 MB please"); return; } const b64 = await toB64(f); setCustomizeForm(p => p ? { ...p, userAvatarBase64: b64, userAvatarPath: null } : p); }} />
                </label>
                <label className="field" style={{ flex: 1 }}><span>Display name</span><input value={customizeForm.userName} placeholder="Your name" onChange={e => setCustomizeForm(p => p ? { ...p, userName: e.target.value } : p)} /></label>
              </div>
            </div>

            {/* AI */}
            <div className="customize-section">
              <span className="customize-section-label">AI</span>
              <div className="customize-row">
                <label className="customize-avatar-upload">
                  <div className="customize-avatar ai">
                    {customizeForm.aiAvatarBase64 ? <img src={`data:image/png;base64,${customizeForm.aiAvatarBase64}`} alt="" />
                      : customizeForm.aiAvatarPath ? <img src={backend.toAssetUrl(customizeForm.aiAvatarPath)} alt="" />
                      : <span>{customizeForm.aiName[0]?.toUpperCase() || "A"}</span>}
                    <div className="customize-avatar-overlay"><Image size={13} /></div>
                  </div>
                  <input hidden type="file" accept="image/*" onChange={async e => { const f = e.target.files?.[0]; if (!f) return; if (f.size > 4*1024*1024) { setToast("Under 4 MB please"); return; } const b64 = await toB64(f); setCustomizeForm(p => p ? { ...p, aiAvatarBase64: b64, aiAvatarPath: null } : p); }} />
                </label>
                <label className="field" style={{ flex: 1 }}><span>AI name</span><input value={customizeForm.aiName} placeholder="Assistant" onChange={e => setCustomizeForm(p => p ? { ...p, aiName: e.target.value } : p)} /></label>
              </div>
            </div>

            {/* Instructions */}
            <label className="field">
              <span>AI instructions <span style={{ opacity: 0.4, fontSize: "0.64rem" }}>(system prompt — tells the AI how to behave)</span></span>
              <textarea rows={4} placeholder="e.g. You are a physics tutor. Always show step-by-step derivations with LaTeX." value={customizeForm.aiInstructions} onChange={e => setCustomizeForm(p => p ? { ...p, aiInstructions: e.target.value } : p)} />
            </label>
            <div className="preset-row">{INSTRUCTION_PRESETS.map(p => <button key={p.label} className="mini-button" type="button" onClick={() => setCustomizeForm(f => f ? { ...f, aiInstructions: p.value } : f)}>{p.label}</button>)}</div>

            <div className="dialog-actions">
              <button className="secondary-button" type="button" onClick={() => setCustomizeForm(null)}>Cancel</button>
              <button className="primary-button" type="submit" disabled={busy || !customizeForm.title.trim()}>Save</button>
            </div>
          </form>
        </div>
      )}

      {/* Toast */}
      {toast && <div className="toast" role="alert" style={toast.startsWith("Send failed") || toast.startsWith("Not saved") ? { borderColor: "var(--danger)", color: "var(--danger)" } : {}}><span>{toast}</span><button className="icon-button" type="button" onClick={() => setToast(null)}><X size={13} /></button></div>}
    </div>
  );
}
