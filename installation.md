# Installation

This repository is meant to store source code, docs, placeholder folders, and configuration samples. Do not commit `AI Chat.exe`, model files, runtime binaries, API keys, chats, logs, backups, or generated media.

The `.exe` is ignored on purpose. Build it locally from the source, or upload it separately as a GitHub Release asset.

## Requirements

Install these before running from source:

- Node.js 20 or newer
- Rust stable
- Tauri v2 system prerequisites for your OS
- Git, if you want to clone from the command line

On Windows, install the Microsoft C++ build tools if Rust or Tauri asks for them.

## Get The Source

With GitHub Desktop:

1. Open GitHub Desktop.
2. Choose `File` -> `Clone repository`.
3. Pick `Kivitas/ai-chat`.
4. Clone it to a normal local folder.
5. Open the folder in a terminal.

With command line:

```sh
git clone https://github.com/Kivitas/ai-chat.git
cd ai-chat
```

## Run In Development

Install dependencies:

```sh
npm install
```

Start the Tauri development app:

```sh
npm run tauri:dev
```

If you only want to check the frontend build:

```sh
npm run build
```

## Build The Windows EXE

Build the frontend and desktop app:

```sh
npm install
npm run build
npm run tauri:build
```

The built executable is created under:

```text
src-tauri/target/release/
```

For a portable folder, copy the built executable to the main app folder and name it:

```text
AI Chat.exe
```

Recommended release layout:

```text
AI Chat/
|-- AI Chat.exe
|-- App/
|-- Data/
|-- Runtimes/
|-- README.md
`-- installation.md
```

## Where To Put Models

Put chat GGUF files here:

```text
Data/Models/
```

The app scans this folder recursively.

Put image model files here:

```text
Data/ImageModels/
```

Put video model files here:

```text
Data/VideoModels/
```

Supported model file extensions include:

```text
.gguf, .safetensors, .ckpt, .onnx, .pt, .pth
```

Do not upload model files to GitHub. They are usually too large and are intentionally ignored.

## Where To Put Runtimes

For local chat with GGUF models, put `llama-cli.exe` here:

```text
Runtimes/Chat/llama-cli.exe
```

For image generation runtimes, put the image runtime executable here:

```text
Runtimes/Image/
```

Recognized Windows image runtime names include:

```text
image-runtime.exe
sd.exe
comfy-portable.exe
```

For video generation runtimes, put the video runtime executable here:

```text
Runtimes/Video/
```

Recognized Windows video runtime name:

```text
video-runtime.exe
```

Runtime binaries should not be committed. Keep only the `.gitkeep` placeholder folders in Git.

## API Keys

Open the app, go to `Settings` -> `Providers`, and add the provider keys you want.

Supported provider key fields:

- OpenAI
- OpenRouter
- Gemini
- Claude
- Mistral

Keys are stored locally under:

```text
Data/Config/Accounts/
```

That folder is ignored and should never be uploaded.

## Uploading To GitHub

Commit source files only.

Before committing, make sure these are not staged:

- `AI Chat.exe`
- `node_modules/`
- `dist/`
- `src-tauri/target/`
- `.gguf` model files
- runtime `.exe` files
- `Data/Config/Accounts/`
- `Data/Chats/`
- `Data/Logs/`
- `Data/Backups/`
- generated media files

In GitHub Desktop:

1. Review the changed files list.
2. Confirm only source/docs/config placeholder files are selected.
3. Commit the changes.
4. Push to GitHub.

## Sharing The EXE

Do not commit the `.exe` into the repository.

Use GitHub Releases instead:

1. Build the app locally.
2. Create a zip folder containing `AI Chat.exe`, `App/`, `Data/`, `Runtimes/`, `README.md`, and `installation.md`.
3. Go to the GitHub repository page.
4. Open `Releases`.
5. Click `Draft a new release`.
6. Create a version tag, for example `v0.1.0`.
7. Upload the zip file as a release asset.
8. Publish the release.

The repository stays clean, and users can download the built app from Releases.

## Validation Commands

Run these before pushing source changes:

```sh
npm run lint
npm run build
npm run validate:repo-files
```

Rust checks:

```sh
cd src-tauri
cargo fmt --check
cargo check
cargo clippy -- -D warnings
```

## Common Problems

If the app says no chat model is available, add a `.gguf` file to `Data/Models/` and click rescan.

If local chat does not run, put `llama-cli.exe` in `Runtimes/Chat/`.

If image or video generation does not run locally, add both a compatible model and a compatible runtime executable.

If provider chat or media generation does not work, add the provider API key and enable remote providers in Settings.

If GitHub Desktop shows large generated files, do not commit them. They belong in local folders or GitHub Releases, not in the source repository.
