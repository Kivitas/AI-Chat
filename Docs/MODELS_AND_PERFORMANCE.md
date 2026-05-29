# Models And Performance

Keep model and runtime files out of Git. The app is designed to load them from local folders at runtime.

## Where To Put Files

- Chat GGUF models: `Data/Models/`
- Image model files: `Data/ImageModels/`
- Video model files: `Data/VideoModels/`
- Chat runtime executable: `Runtimes/Chat/llama-cli.exe` on Windows, or `Runtimes/Chat/llama-cli` on Linux/macOS.
- Image runtime executable: `Runtimes/Image/`
- Video runtime executable: `Runtimes/Video/`

Chat model folders are scanned recursively for `.gguf` files. Image and video folders are scanned for `.gguf`, `.safetensors`, `.ckpt`, `.onnx`, `.pt`, and `.pth` files. Use descriptive filenames so the model picker stays easy to scan.

## Recommended Model Hygiene

- Keep only the models you actually use in the active folders.
- Avoid duplicate copies of the same model under different names.
- Delete stale model files from `Data/Models` when you switch to a new main model.
- Keep backups and generated media out of the model folders.
- Prefer one active chat GGUF per workflow instead of loading many large files at once.

## Performance Tips

- Leave CPU threads blank to let the app use the host defaults.
- Leave GPU/NPU layers blank unless you need to force a specific offload count.
- Use smaller chat models for everyday work and larger models only when the task needs them.
- The app now stays local-first: it prefers the smallest available local GGUF, and only uses a remote provider when you explicitly select one or when no compatible local chat model is available.
- Prompt cache files live under `Data/Logs/PromptCache/` and are reused across the same chat and model.
- The runtime context size is now calculated from the prompt and output size instead of always using a large fixed window.
- Keep your active prompt context short when you want faster replies.
- Trim attachments and logs periodically so the data folder stays responsive.

## Storage And Cleanup

- Do not commit `node_modules/`, `dist/`, `src-tauri/target/`, runtime binaries, or model files.
- Do not commit generated executables, account folders, API keys, encrypted chats, local config, logs, backups, or generated media.
- Keep `Data/Backups/`, `Data/Logs/`, and `Data/Media/` for generated local content only.
- Use `npm run validate:repo-files` before pushing changes.

## Provider Chat

- Save API keys in Settings for OpenAI, OpenRouter, Gemini, Claude, or Mistral.
- Type a new key to add or replace a saved provider key.
- Leave the field blank to keep the saved key.
- Use the delete button beside a provider and save to remove that provider key.
- When remote provider support is enabled, the app exposes provider default chat models explicitly in the model list.
- Local GGUF remains the default choice for new chats when one is available.

## Media Generation

- Chat GGUF models are separate from image and video generation.
- Image and video model folders may also contain `.gguf` files if your selected media runtime supports them.
- Image and video generation use dedicated runtime executables in `Runtimes/Image/` and `Runtimes/Video/`.
- The runtime receives a job JSON plus `AI_CHAT_*` environment variables and must write the generated file into the output directory.
- The app imports the output file into `Data/Media/Images` or `Data/Media/Videos` after the runtime completes.

## Portable Paths

The app resolves paths from `${APP_ROOT}` and `${DATA_DIR}`. Keep path templates variable-based so the repository stays portable across machines and USB installs.
