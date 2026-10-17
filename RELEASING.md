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

## Protocol changes

`shared/src/lib.rs` defines the wire protocol version. Bump `PROTOCOL_VERSION` only for breaking changes to paths, payloads, auth, or semantics. UI-only releases must not bump it. If the server stops accepting older clients, bump `MIN_CLIENT_PROTOCOL_VERSION` in `server/src/constants.rs` as well.

For a protocol bump, release and deploy the server first. A new client refuses to sync with a server that does not expose `GET /version`. Verify the deployed server before tagging the client:

```bash
curl https://nooto.nextmiracle.eu/version
```