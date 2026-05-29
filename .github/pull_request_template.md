## Summary


## Validation

- [ ] `npm run lint`
- [ ] `npm run build`
- [ ] `npm run validate:repo-files`
- [ ] `npm run tauri:build -- --no-bundle` when desktop packaging/runtime code changed
- [ ] `cargo fmt --check` from `src-tauri`
- [ ] `cargo check --locked` from `src-tauri`

## Notes

Confirm that no generated folders, runtime binaries, models, encrypted data, local config, or machine-specific absolute paths are included.
