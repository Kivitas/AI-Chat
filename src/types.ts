export type ViewId =
  | "chat"
  | "models"
  | "gallery"
  | "settings"
  | "backups"
  | "diagnostics";

export type ChatRole = "system" | "user" | "assistant" | "tool" | "media";
export type MediaType = "image" | "video" | "file";
export type GenerationStatus = "pending" | "ready" | "failed";

export interface PathConfig {
  appRoot: string;
  dataDir: string;
  modelsDir: string;
  imageModelsDir: string;
  videoModelsDir: string;
  charactersDir: string;
  chatsDir: string;
  mediaDir: string;
  imagesDir: string;
  videosDir: string;
  thumbnailsDir: string;
  configDir: string;
  profilesDir: string;
  presetsDir: string;
  backupsDir: string;
  logsDir: string;
  runtimesDir: string;
}

export interface AppConfig {
  appName: string;
  portable: boolean;
  paths: PathConfig;
  limits: {
    maxRepoFileSizeMb: number;
  };
  appearance: {
    themeId: string;
    themeGroup: "all" | "light" | "dark" | "night";
    smooth: boolean;
    compact: boolean;
    autoScroll: boolean;
    typewriter: boolean;
    showPerformance: boolean;
    wideMessages: boolean;
    technicalMode: boolean;
    scale: number;
    messageStyle: "cards" | "bubbles" | "minimal";
    sidebarCollapsed: boolean;
  };
  privacy: {
    localOnly: boolean;
    telemetry: boolean;
    analytics: boolean;
    cloudSync: boolean;
    remoteProviders: boolean;
  };
  security: {
    lockOnStartup: boolean;
    autoLockMinutes: number;
  };
  generation?: {
    threads?: number | null;
    gpuLayers?: number | null;
    maxTokens?: number | null;
  } | null;
}

export type ResolvedDirectories = PathConfig;

export interface AuthCapabilities {
  password: boolean;
  passkey: boolean;
  biometric: boolean;
}

export interface ProfileRecord {
  id: string;
  displayName: string;
  avatarPath?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CharacterRecord {
  id: string;
  name: string;
  description: string;
  styleNotes: string;
  avatarPath?: string | null;
  defaultModelId?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ModelRecord {
  id: string;
  name: string;
  path: string;
  kind: "chat" | "image" | "video";
  sizeBytes: number;
  lastModified?: string | null;
  available: boolean;
  favorite: boolean;
  missing: boolean;
}

export interface RuntimeRecord {
  id: string;
  kind: "chat" | "image" | "video";
  name: string;
  path: string;
  available: boolean;
  note: string;
}

export interface ProviderStatus {
  id: "openai" | "openrouter" | "gemini" | "claude" | "mistral";
  label: string;
  defaultModelId: string;
  hasKey: boolean;
  available: boolean;
  note: string;
}

export interface MediaAsset {
  id: string;
  kind: MediaType;
  path: string;
  thumbnailPath?: string | null;
  contentPreview?: string | null;
  prompt?: string | null;
  status?: GenerationStatus | null;
  modelUsed?: string | null;
  seed?: number | null;
  sourceLabel?: string | null;
}

export interface ChatMessage {
  id: string;
  role: ChatRole;
  text: string;
  createdAt: string;
  pinned: boolean;
  attachments: MediaAsset[];
}

export interface ChatSettings {
  modelId?: string | null;
  imageModelId?: string | null;
  videoModelId?: string | null;
  profileOverrideId?: string | null;
  userName?: string | null;
  aiName?: string | null;
  aiInstructions?: string | null;
  userAvatar?: string | null;
  aiAvatar?: string | null;
}

export interface ChatRecord {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  favorite: boolean;
  archived: boolean;
  settings: ChatSettings;
  messages: ChatMessage[];
}

export interface DiagnosticsReport {
  appRoot: string;
  dataDir: string;
  runtimesDir: string;
  detectedModels: ModelRecord[];
  missingModelPaths: string[];
  runtimeAvailability: RuntimeRecord[];
  diskUsageBytes: number;
  repoOversizeWarnings: string[];
}

export interface BackupRecord {
  id: string;
  path: string;
  createdAt: string;
  sizeBytes: number;
  filesCopied: number;
}

export interface BackupResult {
  snapshot: AppSnapshot;
  backup: BackupRecord;
}

export interface AppSnapshot {
  appName: string;
  setupComplete: boolean;
  locked: boolean;
  offlineNotice: string;
  config: AppConfig;
  directories: ResolvedDirectories;
  authCapabilities: AuthCapabilities;
  profile?: ProfileRecord | null;
  character?: CharacterRecord | null;
  chats: ChatRecord[];
  activeChatId?: string | null;
  chatModels: ModelRecord[];
  imageModels: ModelRecord[];
  videoModels: ModelRecord[];
  providers: ProviderStatus[];
  runtimes: RuntimeRecord[];
}

export interface SetupPayload {
  username: string;
  password: string;
  displayName: string;
  profileAvatarBase64?: string | null;
  characterName: string;
  characterDescription: string;
  characterStyleNotes: string;
  characterAvatarBase64?: string | null;
}

export interface MediaPromptPreview {
  prompt: string;
  sources: string[];
}

export interface SettingsUpdatePayload {
  paths: PathConfig;
  autoLockMinutes: number;
  appearance: {
    themeId: string;
    themeGroup: "all" | "light" | "dark" | "night";
    smooth: boolean;
    compact: boolean;
    autoScroll: boolean;
    typewriter: boolean;
    showPerformance: boolean;
    wideMessages: boolean;
    technicalMode: boolean;
    scale: number;
    messageStyle: "cards" | "bubbles" | "minimal";
    sidebarCollapsed: boolean;
  };
  generation?: {
    threads?: number | null;
    gpuLayers?: number | null;
    maxTokens?: number | null;
  } | null;
  remoteProviders: boolean;
  providerKeys?: {
    openaiKey?: string | null;
    openrouterKey?: string | null;
    geminiKey?: string | null;
    claudeKey?: string | null;
    mistralKey?: string | null;
  } | null;
}

export interface ChatSettingsUpdatePayload {
  chatId: string;
  modelId?: string | null;
  imageModelId?: string | null;
  videoModelId?: string | null;
  profileOverrideId?: string | null;
  userName?: string | null;
  aiName?: string | null;
  aiInstructions?: string | null;
  userAvatarBase64?: string | null;
  aiAvatarBase64?: string | null;
  userAvatarPath?: string | null;
  aiAvatarPath?: string | null;
}
