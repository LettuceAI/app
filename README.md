<p align="center">
  <img src="apps/tauri/icons/128x128.png" alt="LettuceAI logo" width="88" height="88" />
</p>

# LettuceAI: AI Character Chat and Roleplay

LettuceAI is an open-source AI roleplay and character chat app for Android, Windows,
macOS, and Linux. Create characters, talk with them individually or in groups,
and branch conversations to try different directions in a story.

Chats, characters, and memories are stored on your device. You can connect your own
AI provider accounts or run models locally on desktop. When using a remote provider,
the conversation context is sent to that provider.

[Download LettuceAI](https://www.lettuceai.app) ·
[Documentation](https://www.lettuceai.app/docs) ·
[Discord](https://discord.gg/745bEttw2r)

## Features

- Character and persona editors, scenes, and custom system prompts.
- Group chats with roleplay and conversation modes and speaker selection.
- Alternate replies and conversation branches, including branches to other characters or groups.
- Long-term chat memory, conversation summaries, and keyword-triggered lorebooks.
- Image generation, speech recognition, and text-to-speech.
- Character card imports, library exports, and backups.

## AI providers and local LLMs

LettuceAI works with OpenAI, Anthropic, Google Gemini, OpenRouter, DeepSeek, and other
providers. It also connects to Ollama, LM Studio, and custom OpenAI-compatible or
Anthropic endpoints.

Desktop builds can run local LLMs from GGUF files through llama.cpp, with CUDA,
Vulkan, or Metal acceleration. Image generation supports remote providers, ComfyUI,
and a local stable-diffusion.cpp engine.

## Building from source

The backend is written in Rust, with a React frontend and a Tauri 3 alpha shell. You need
Bun 1.4.3, Rust 1.98 or newer, CMake, a C/C++ toolchain, and Tauri's native dependencies
for your platform. Linux builds require the WebKitGTK 4.1 development packages.

Cargo fetches the Tauri and plugin forks from GitHub at the commit revisions pinned
in [Cargo.toml](Cargo.toml).

Then, from the repository root:

```sh
bun run setup
bun run dev
```

Use `bun run build` to create a desktop bundle. GPU builds have `build:cuda`,
`build:vulkan`, and `build:metal` scripts. For Android, install the SDK, NDK, and JDK,
then run `bun run tauri android dev` or `bun run tauri android build`.
iOS requires native project initialization; see the [shell README](apps/tauri/README.md).

For frontend work, `bun run dev:ui` runs the UI in a browser with a mock backend.
The frontend lives in [`apps/ui/`](apps/ui/), the native shell in
[`apps/tauri/`](apps/tauri/), and the Rust libraries in [`crates/`](crates/).

Release workflows, signing secrets, and AUR publication are documented in
[Releasing](.github/RELEASING.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines and required checks.
Open an issue before starting a large feature or changing how user data is stored.

For bug reports, include your app version, platform, and steps to reproduce the
problem in the [issue tracker](https://github.com/LettuceAI/mobile-app/issues).
Remove credentials and private chat content from any logs you attach.

## License

[GNU Affero General Public License v3.0](LICENSE).
