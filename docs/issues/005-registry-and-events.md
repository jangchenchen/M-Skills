# Issue 005: Registry Cache And Tauri Events

## What To Build

Add SQLite persistence and frontend update events so the app feels fast while still treating the filesystem as the source of truth.

## Acceptance Criteria

- [ ] Registry stores artifacts, installations, source metadata, status, installed version, and on-disk path.
- [ ] Startup scan upserts registry state from adapters.
- [ ] Install and uninstall update the registry only after filesystem success.
- [ ] Tauri commands expose list, import preview, install, and uninstall operations.
- [ ] Tauri events notify the frontend of scan progress and changed installations.
- [ ] Tests cover registry rebuild from scan results.

## Blocked By

Issues 001, 002, and 004.

