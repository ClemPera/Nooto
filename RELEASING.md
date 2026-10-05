# Releasing

Here are the steps to make a release.

## Client

1. Set the new version in `client/src-tauri/Cargo.toml`.
2. Set the same version in `client/src-tauri/tauri.conf.json`.
3. Run `npm run tauri android build` from `client/`. This updates `client/src-tauri/gen/android/app/tauri.properties`.
4. Add a changelog file in `fastlane/metadata/android/en-US/changelogs/`, named after the `versionCode` in `tauri.properties`.
5. Commit the changes.
6. Tag and push:

```bash
   git tag client-vX.Y.Z
   git push origin main client-vX.Y.Z
```

## Server

Tag and push, nothing else is needed:

```bash
git tag server-vX.Y.Z
git push origin main server-vX.Y.Z
```