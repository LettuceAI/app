# lettuce-speech

Owns speech recognition and synthesis workflows, voice configuration, learned
corrections, local speech runtimes, and typed audio results.

ASR and TTS remain separate internal modules. Persistent audio is handed to
`lettuce-media`; model artifacts and long-running work use injected ports.
