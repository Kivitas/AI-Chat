# Security

Security fixes are handled on `main` until there are tagged releases.

## Reporting

Please report security issues privately to the repository owner. Do not open a public issue for problems involving account encryption, provider keys, local file access, command execution, or runtime loading.

## Project Assumptions

- The app is a local desktop app.
- Chats are encrypted on disk after setup.
- Provider keys are saved locally under the account config.
- Remote providers are disabled unless the user enables them.
- Model runtimes are supplied by the user and run from the runtime folders.

Do not upload local account data, keys, chats, logs, backups, model files, runtime binaries, or generated media.
