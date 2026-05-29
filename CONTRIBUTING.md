# Contributing

Thanks for taking a look at the project.

This app is local-first, so changes should keep that direction. Avoid adding hosted services, analytics, telemetry, or hidden provider fallbacks. Remote providers are fine when the user enables them and saves a key.

## Before A Pull Request

Run the usual checks:

```sh
npm run lint
npm run build
npm run validate:repo-files
cd src-tauri
cargo fmt --check
cargo check
cargo clippy -- -D warnings
```

## Keep Out Of Commits

Please do not commit:

- `node_modules`, `dist`, or `src-tauri/target`
- built executables or installers
- model files
- runtime binaries
- generated media
- logs, backups, accounts, chats, API keys, or local config

Use portable paths such as `${APP_ROOT}`, `${DATA_DIR}`, and `${MEDIA_DIR}` instead of machine-specific absolute paths.

The source is GPL-3.0-only.
