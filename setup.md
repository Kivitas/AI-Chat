# Windows Setup

This guide is for Windows users who want to build their own `AI Chat.exe` from the source code.

GitHub can host executable files, but the clean way to share a built app is through GitHub Releases, usually as a zip file. The source repository should still stay focused on source code, docs, placeholder folders, and sample config.

macOS support is planned for upcoming updates.

## Why Build Your Own EXE

Windows SmartScreen is most aggressive with downloaded unsigned executables from the internet. If you build the app locally, the executable is created on your machine instead of downloaded from the web, so it usually avoids the common SmartScreen download warning.

For public downloads, the strongest fix is still proper code signing plus release reputation over time. A locally built executable helps individual Windows users run the app without relying on a downloaded unsigned binary.

## Requirements

Install these first:

- Node.js 20 or newer
- Rust stable
- Microsoft C++ Build Tools
- WebView2 Runtime
- Git or GitHub Desktop

Tauri v2 uses these tools to compile the desktop app.

## Clone The Project

With GitHub Desktop:

1. Open GitHub Desktop.
2. Choose `File` -> `Clone repository`.
3. Select `Kivitas/ai-chat`.
4. Clone the repository to a local folder.
5. Open that folder in PowerShell or Windows Terminal.

With command line:

```powershell
git clone https://github.com/Kivitas/ai-chat.git
cd ai-chat
```

## Install Dependencies

Run this from the project root:

```powershell
npm install
```

## Run The App In Development

Use this while testing changes:

```powershell
npm run tauri:dev
```

This opens the desktop app in development mode.

## Build Your Own EXE

Run these commands from the project root:

```powershell
npm run build
npm run tauri:build
```

The release executable is created here:

```text
src-tauri/target/release/ai-chat.exe
```

For the portable folder layout, copy that file to the project root or release folder and rename it:

```text
AI Chat.exe
```

Recommended folder layout:

```text
AI Chat/
|-- AI Chat.exe
|-- App/
|-- Data/
|-- Runtimes/
|-- README.md
|-- installation.md
`-- setup.md
```

## First Run

1. Double-click `AI Chat.exe`.
2. Create your local account.
3. Put chat `.gguf` models in `Data/Models/`.
4. Put `llama-cli.exe` in `Runtimes/Chat/` if you want local GGUF chat.
5. Open Settings and click rescan.

## Model And Runtime Folders

Chat models:

```text
Data/Models/
```

Image models:

```text
Data/ImageModels/
```

Video models:

```text
Data/VideoModels/
```

Chat runtime:

```text
Runtimes/Chat/llama-cli.exe
```

Image runtime:

```text
Runtimes/Image/
```

Video runtime:

```text
Runtimes/Video/
```

Do not commit model files or runtime binaries unless you intentionally want to distribute them separately. They are usually large and machine-specific.

## Sharing A Windows Build On GitHub

The recommended GitHub flow is:

1. Build `AI Chat.exe` locally.
2. Create a zip containing the portable app folder.
3. Go to the repository on GitHub.
4. Open `Releases`.
5. Draft a new release.
6. Use a tag such as `v1.0.1`.
7. Upload the zip as a release asset.
8. Publish the release.

This keeps the source repository clean while still letting users download the Windows build from GitHub.

## SmartScreen Notes

Building locally is the easiest path for users who want to avoid warnings caused by downloading an unknown unsigned executable.

If you distribute a prebuilt exe or zip, Windows may still warn users until the file is code-signed and has enough reputation. That is normal for new Windows desktop apps.

## Validation Before Release

Run these before creating a release:

```powershell
npm run lint
npm run build
npm run validate:repo-files
```

Rust checks:

```powershell
cd src-tauri
cargo fmt --check
cargo check
cargo clippy -- -D warnings
```

Then build:

```powershell
cd ..
npm run tauri:build
```
