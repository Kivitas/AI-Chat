# AI Chat

A desktop chat app for running local GGUF models, with optional provider keys when you want to use cloud models too.

The app is built with Tauri, React, and Rust. It is meant to run from one folder, keep chats on disk, and avoid sending anything out unless remote providers are enabled in settings.

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![CI](https://github.com/Kivitas/ai-chat/actions/workflows/ci.yml/badge.svg)](https://github.com/Kivitas/ai-chat/actions/workflows/ci.yml)

## What It Does

- Runs local `.gguf` chat models through `llama-cli`.
- Keeps chats encrypted after setup.
- Supports Markdown, code highlighting, and KaTeX math.
- Lets each chat use its own model, names, avatars, and instructions.
- Has incognito chats that are not saved.
- Can use OpenAI, OpenRouter, Gemini, Claude, or Mistral if you add keys.
- Has settings, themes, diagnostics, backups, and a media gallery.
- Can hand off image or video generation jobs to local runtimes, or use OpenAI when provider media generation is enabled.

## Folder Layout

In a release folder, keep the exe at the top level:

```text
AI Chat/
|-- AI Chat.exe
|-- App/
|   `-- WebView2Profile/
|-- Data/
|   |-- Models/
|   |-- ImageModels/
|   |-- VideoModels/
|   |-- Chats/
|   |-- Profiles/
|   |-- Characters/
|   |-- Media/
|   |-- Config/
|   |-- Presets/
|   |-- Backups/
|   `-- Logs/
|-- Runtimes/
|   |-- Chat/
|   |-- Image/
|   `-- Video/
`-- README.md
```

`Data/Models/` is for chat `.gguf` files. The app scans it recursively.

`Runtimes/Chat/` is where `llama-cli.exe` goes on Windows. On Linux or macOS, put the executable there as `llama-cli`.

Image and video generation need their own runtime executables in `Runtimes/Image/` and `Runtimes/Video/`. Their model files go in `Data/ImageModels/` and `Data/VideoModels/`.

## API Keys

Open Settings, go to Providers, and add the keys you want.

- OpenAI is used for OpenAI chat, image generation, and Sora video generation.
- OpenRouter is available for chat models through OpenRouter.
- Gemini, Claude, and Mistral are available for chat.

Keys are stored in the local account config under `Data/Config/Accounts/`. Do not upload that folder.

## Development

Required tools:

- Node.js 20+
- Rust stable
- Tauri v2 prerequisites for your OS

Common commands:

```sh
npm install
npm run lint
npm run build
npm run tauri:dev
```

Rust checks:

```sh
cd src-tauri
cargo fmt --check
cargo check
cargo clippy -- -D warnings
```

Repository check:

```sh
npm run validate:repo-files
```

## Notes Before Uploading

Do not upload local accounts, API keys, encrypted chats, generated media, model files, runtime binaries, build folders, or `node_modules`.

Large local files to keep out of the repository include `.gguf`, `.safetensors`, `.ckpt`, `.onnx`, `.pt`, `.pth`, `.bin`, `.mp4`, `.webm`, `.exe`, `.dll`, and installer files.

## More Docs

- Model and performance notes: [Docs/MODELS_AND_PERFORMANCE.md](Docs/MODELS_AND_PERFORMANCE.md)
- Screenshot notes: [Docs/github-screenshots](Docs/github-screenshots/README.md)

## License

GPL-3.0-only. See [LICENSE](LICENSE).
