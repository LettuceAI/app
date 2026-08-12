# i18n Tools

Reusable scripts for the locale translation checkpoint workflow.

These scripts use `.i18n-translate` as the default working directory. Set `I18N_WORKDIR`
to point at a different checkpoint directory.

Run with Bun from the repo root:

```bash
bun scripts/i18ntools/status.ts
bun scripts/i18ntools/analyze-missing.ts
bun scripts/i18ntools/split-missing.ts
bun scripts/i18ntools/validate-done.ts tr-1 tr-2
bun scripts/i18ntools/refresh-suspects.ts tr-1 tr-2
COMPLETE_ONLY=1 bun scripts/i18ntools/merge-locales.ts
```

`COMPLETE_ONLY=1` makes the merge skip locales unless both checkpoint halves are complete.
This is the safe default for updating `src/core/i18n/locales/*.ts` from translated JSON.

The Google AI Studio generation loop still lives in `.i18n-translate/google_ai_translate.py`.
A TypeScript port can be added here later without changing the checkpoint file layout.

