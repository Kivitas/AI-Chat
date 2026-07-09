/**
 * backend.ts
 *
 * Thin wrapper around Tauri's invoke() for all IPC calls to the Rust backend.
 * This file contains no hosted fallback and no mock data.
 * The app is a Tauri desktop application. Durable logic lives in src-tauri/src/.
 */
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type {
  AppSnapshot,
  BackupRecord,
  BackupResult,
  ChatMessage,
  ChatSettingsUpdatePayload,
  DiagnosticsReport,
  FolderContext,
  MediaPromptPreview,
  SettingsUpdatePayload,
  SetupPayload,
} from "../types";


export const backend = {
  // ── App state ─────────────────────────────────────────────────────────────
  getAppState(): Promise<AppSnapshot> {
    return invoke("get_app_state");
  },

  // ── Auth ──────────────────────────────────────────────────────────────────
  initializeSetup(payload: SetupPayload): Promise<AppSnapshot> {
    return invoke("initialize_setup", { payload });
  },

  unlockWithPassword(username: string, password: string): Promise<AppSnapshot> {
    return invoke("unlock_with_password", { username, password });
  },

  lock(): Promise<AppSnapshot> {
    return invoke("lock_app");
  },

  // ── Profile / character ───────────────────────────────────────────────────
  saveProfile(payload: { displayName: string; avatarBase64?: string | null }): Promise<AppSnapshot> {
    return invoke("save_profile", { payload });
  },

  saveCharacter(payload: { name: string; description: string; styleNotes: string; avatarBase64?: string | null }): Promise<AppSnapshot> {
    return invoke("save_character", { payload });
  },

  // ── Chats ─────────────────────────────────────────────────────────────────
  createChat(title?: string): Promise<AppSnapshot> {
    return invoke("create_chat", { title: title ?? null });
  },

  selectChat(chatId: string): Promise<AppSnapshot> {
    return invoke("select_chat", { chatId });
  },

  renameChat(chatId: string, title: string): Promise<AppSnapshot> {
    return invoke("rename_chat", { chatId, title });
  },

  toggleChatFlag(chatId: string, field: "favorite" | "archived"): Promise<AppSnapshot> {
    return invoke("toggle_chat_flag", { chatId, field });
  },

  clearChatMessages(chatId: string): Promise<AppSnapshot> {
    return invoke("clear_chat_messages", { chatId });
  },

  deleteChat(chatId: string): Promise<AppSnapshot> {
    return invoke("delete_chat", { chatId });
  },

  setChatSettings(payload: ChatSettingsUpdatePayload): Promise<AppSnapshot> {
    return invoke("set_chat_settings", { payload });
  },

  // ── Messages ──────────────────────────────────────────────────────────────
  sendChatMessage(chatId: string, text: string): Promise<AppSnapshot> {
    return invoke("send_chat_message", { chatId, text });
  },

  /** Incognito: uses real inference but result is not persisted to disk. */
  sendIncognitoChatMessage(chatId: string, text: string): Promise<ChatMessage> {
    return invoke("send_incognito_chat_message", { chatId, text });
  },

  deleteMessage(chatId: string, messageId: string): Promise<AppSnapshot> {
    return invoke("delete_message", { chatId, messageId });
  },

  togglePinnedMessage(chatId: string, messageId: string): Promise<AppSnapshot> {
    return invoke("toggle_pinned_message", { chatId, messageId });
  },

  updateMessage(chatId: string, messageId: string, text: string): Promise<AppSnapshot> {
    return invoke("update_message", { chatId, messageId, text });
  },

  // ── Media ─────────────────────────────────────────────────────────────────
  importMediaAttachment(chatId: string, fileName: string, base64: string, kind: string): Promise<AppSnapshot> {
    return invoke("import_media_attachment", { chatId, fileName, base64, kind });
  },

  buildMediaPromptFromChat(chatId: string, userPrompt: string): Promise<MediaPromptPreview> {
    return invoke("build_media_prompt_from_chat", { chatId, userPrompt });
  },

  generateImageFromChat(chatId: string, prompt: string): Promise<AppSnapshot> {
    return invoke("generate_image_from_chat", { chatId, prompt });
  },

  generateVideoFromChat(chatId: string, prompt: string): Promise<AppSnapshot> {
    return invoke("generate_video_from_chat", { chatId, prompt });
  },

  // ── Models ────────────────────────────────────────────────────────────────
  rescanModels(): Promise<AppSnapshot> {
    return invoke("rescan_models");
  },

  // ── Settings ──────────────────────────────────────────────────────────────
  saveSettings(payload: SettingsUpdatePayload): Promise<AppSnapshot> {
    return invoke("save_settings", { payload });
  },

  // ── Diagnostics ───────────────────────────────────────────────────────────
  diagnostics(): Promise<DiagnosticsReport> {
    return invoke("get_diagnostics");
  },

  // ── Backups ───────────────────────────────────────────────────────────────
  listBackups(): Promise<BackupRecord[]> {
    return invoke("list_backups");
  },

  createBackup(): Promise<BackupResult> {
    return invoke("create_backup");
  },

  // ── Folder context ────────────────────────────────────────────────────────
  /** Set the workspace folder path for a chat (saved per-chat persistently). */
  setChatFolder(chatId: string, folderPath: string): Promise<AppSnapshot> {
    return invoke("set_chat_folder", { chatId, folderPath });
  },

  /** Remove the workspace folder from a chat. */
  clearChatFolder(chatId: string): Promise<AppSnapshot> {
    return invoke("clear_chat_folder", { chatId });
  },

  /** Scan a folder and return its file tree + contents for the workspace panel. */
  readChatFolder(folderPath: string): Promise<FolderContext> {
    return invoke("read_chat_folder", { folderPath });
  },

  /** Apply tool calls embedded in an assistant message to the workspace folder. */
  applyChatTools(chatId: string, messageId: string): Promise<AppSnapshot> {
    return invoke("apply_chat_tools", { chatId, messageId });
  },

  // ── Asset URLs ────────────────────────────────────────────────────────────

  /**
   * Convert a filesystem path from the Rust backend into a URL the
   * Tauri WebView can load. Uses Tauri's convertFileSrc for security.
   */
  toAssetUrl(path: string): string {
    if (!path || path.startsWith("pending://") || path.startsWith("memory://")) return "";
    return convertFileSrc(path);
  },
};
