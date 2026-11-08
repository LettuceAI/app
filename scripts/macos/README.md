# Persistent macOS signing without an Apple account

Keychain uses an app's designated requirement to recognize it. An ad hoc signature pins one build's hash. Signing every build with the same self-signed certificate and bundle identifier gives Keychain a persistent identity without buying an Apple Developer membership. The certificate contains only `CN=LettuceAI Code Signing`; it needs no personal name, address, email, or Apple account. Apple documents this behavior under [Understanding the Code Signature](https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/AboutCS/AboutCS.html).

This provides Keychain identity continuity, not Gatekeeper approval or notarization. Users may still need to approve opening the downloaded app. No root certificate is installed on users' Macs. Existing Keychain items created by an ad hoc build can require a fresh grant when switching to the persistent identity; subsequent releases must keep that identity. Deleted credentials still need to be restored or entered again.

## One-time setup

The locally generated identity is in `.tmp/macos-signing` in the original checkout, which Git ignores. Keep a private backup of the certificate, encrypted private key, and password. Never upload them as release assets or commit them.

For a new project identity only, generate one into a new directory:

```bash
mkdir -p .tmp
bash scripts/macos/create-signing-identity.sh .tmp/macos-signing
```

The script refuses to overwrite an existing directory. Do not regenerate the certificate for each release: changing it invalidates the identity that existing Keychain grants recognize. The public fingerprint in `scripts/macos/signing.sha1` must match the saved certificate.

Configure two repository secrets once, using the generated files as standard input so their values do not enter shell history:

```bash
gh secret set MACOS_SIGNING_CERTIFICATE --repo LettuceAI/mobile-app < .tmp/macos-signing/certificate.base64
gh secret set MACOS_SIGNING_PASSWORD --repo LettuceAI/mobile-app < .tmp/macos-signing/password.txt
```

## Release checks

Run **Build and release** from `main` and leave `source_ref` set to `refactor/lettuceai-next` to build the new app. The prepare job resolves that source to one commit. Desktop and Android jobs check out that commit as the application, and check out the controller workflow's commit separately under `.release-ci` for shared actions and signing scripts. Updating signing and release configuration on `main` therefore does not require merging the refactor application into `main`. The existing legacy desktop workflows are separate.

The release workflow fails early when signing secrets are missing. On each macOS runner, `import-signing-identity.sh` checks the certificate fingerprint before importing its private key. It creates an isolated keychain and grants code-signing trust only on that disposable runner. Tauri receives the imported certificate fingerprint through `APPLE_SIGNING_IDENTITY`; its Apple-specific certificate importer is bypassed. Cleanup removes the runner trust and keychain even when packaging fails.

`test-signing-identity.sh` builds and signs two different executables at the same path using the release certificate and entitlements. Build 1 creates a canary credential; build 2 must read and delete it with Keychain user interaction disabled. Their designated requirements must match. The test uses only the temporary CI keychain and never accesses user credentials.

`verify-signing-identity.sh` verifies the finished app's signature, identifier, and pinned certificate, and rejects a designated requirement containing a build hash. The certificate fingerprint and signing requirement are public; the private key stays in the runner keychain until cleanup.

Linux can validate certificate generation, shell syntax, workflow wiring, and rejection of missing or mismatched credentials. Signing, the Keychain round trip, and the installed app's behavior need a macOS runner. Before publishing the first release, confirm the native smoke test passes and check access again after updating an installed build.
