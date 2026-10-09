# LettuceAI UI parity

Provider control failures use localized typed categories, including InUse and Malformed. Legacy provider/model editors displayed backend error strings (old-code/src/ui/pages/settings/hooks/useProvidersPageController.ts:298-320; old-code/src/ui/pages/settings/hooks/useModelEditorController.ts:1564-1577). Draft verification details retain the redacted provider message for a future editor flow.

Generated provider contracts expose stored-certificate validity with an InvalidPem reason, CertificateAlreadyImported details with the existing id, and optional redacted verification messages with MissingApiKey or InvalidApiKey reasons.

Locale moves from legacy webview localStorage to device UI state (`old-code/src/core/i18n/context.tsx:14,67-82`). A readable legacy key seeds the device choice; unavailable storage falls through to browser-language detection and then English (`old-code/src/core/i18n/context.tsx:62-77`). Persisting the detected device choice is best-effort. Only English resources are currently installed, so other retained locale choices use i18next's English fallback until their resources are added.

Filter refresh uses ContentFilterHit rather than the legacy development panel's five-second interval (`old-code/src/ui/pages/settings/SecurityPage.tsx:101-110`). Invalid legacy locale identifiers fall through to browser detection instead of being persisted as a choice, matching the legacy supported-locale check (`old-code/src/core/i18n/context.tsx:20-21,67-82`; registry at `old-code/src/core/i18n/locales/registry.ts:56-76`).
