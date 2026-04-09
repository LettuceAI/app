<div align="center">
  <img src="https://github.com/LettuceAI/.github/blob/main/profile/LettuceAI-banner.png" alt="LettuceAI Banner" />


   
  # LettuceAI
  
  Privacy-first AI roleplay & storytelling app with long-term memory, custom characters, and 20+ providers. Runs on Android, Windows, macOS, and Linux.
  
  [Overview](#overview) • [Install](#install) • [Development](#development) • [Android](#android) • [iOS](#ios) • [Contributing](#contributing)
</div>

## Overview

  LettuceAI is a free, open-source AI chat app built with Tauri v2, React, and TypeScript. It keeps all chats, characters, and API keys on your device — nothing is
  sent to us. Bring your own keys from OpenAI, Anthropic, Google Gemini, DeepSeek, Mistral, Groq, or any of 20+ supported providers, or run local models with Ollama
  and llama.cpp.

## Screenshots

### Core Experience

| Chat | Character Editor |
| --- | --- |
| ![Chat screen](docs/readme/chat.png) | ![Character editor](docs/readme/character_editor.png) |
| Live roleplay chat with character-aware UI. | Build and refine character identity, definition, and avatar. |

| Memory | Image Generation |
| --- | --- |
| ![Memory screen](docs/readme/memory.png) | ![Image prompt and result](docs/readme/image_prompt.png) |
| Review context summaries and manage saved memories. | Generate character visuals directly from prompts. |

### Advanced Controls

| Models | System Prompt Editor |
| --- | --- |
| ![Models screen](docs/readme/models.png) | ![System prompt editor](docs/readme/system_prompt_editor.png) |
| Configure local or remote model backends. | Edit structured prompt templates and variables. |

Screenshots feature “King Cassian” by [jawawgf](https://character-tavern.com/character/jawawgf/king_cassian), used for demonstration.

## Install

### Prerequisites

- Bun 1.1+ (includes Node.js compatibility): https://bun.sh/
- Rust 1.70+ and Cargo
- Android SDK (optional, for Android builds)
- Xcode + iOS SDK (optional, for iOS builds, macOS only)

### Quick Start

```bash
# Clone the repository
git clone https://github.com/LettuceAI/mobile-app.git
cd mobile-app

# Install dependencies
bun install
```

## Development

### Common Commands

```bash
# Desktop (Tauri)
bun run tauri dev
bun run tauri build
bun run tauri:build:macos

# Desktop with NVIDIA CUDA llama.cpp acceleration
bun run tauri dev --features llama-gpu-cuda
bun run tauri build --features llama-gpu-cuda

# Desktop with NVIDIA CUDA llama.cpp acceleration (auto-detect local GPU arch)
bun run tauri:dev:cuda:auto
bun run tauri:build:cuda:auto

# Desktop with Vulkan llama.cpp acceleration (AMD/Intel/NVIDIA, driver-dependent)
bun run tauri dev --features llama-gpu-vulkan
bun run tauri build --features llama-gpu-vulkan

# Desktop with Metal llama.cpp acceleration (Apple Silicon/Intel Macs, macOS only)
bun run tauri:dev:metal
bun run tauri:build:metal

# Android
bun run tauri android dev
bun run tauri android build

# Quality
bunx tsc --noEmit
bun run check
```

## Android

### Setup

- Install Android Studio and set up the SDK
- Ensure `ANDROID_SDK_ROOT` is set in your environment
- Add platform tools to your `PATH` (example: `export PATH=$ANDROID_SDK_ROOT/platform-tools:$PATH`)

### Build and Run

```bash
# Run on Android emulator
bun run tauri android dev

# Build Android APK
bun run tauri android build
```

## iOS

### Setup (macOS only)

- Install Xcode from the App Store
- Install Xcode command-line tools: `xcode-select --install`
- Install CocoaPods: `sudo gem install cocoapods` (or Homebrew)
- Provide ONNX Runtime for iOS with CoreML support:
  - Build/download an iOS-compatible ONNX Runtime package that includes CoreML EP
  - Set `ORT_LIB_LOCATION` to the directory containing the ONNX Runtime libraries before building
- Initialize iOS project files:

```bash
export ORT_LIB_LOCATION=/absolute/path/to/onnxruntime/ios/libs
bun run tauri ios init
```

### Build and Run

```bash
# Run on iOS simulator/device (from macOS)
bun run tauri ios dev

# Build iOS app
bun run tauri ios build
```

For `llama-gpu-cuda`, install the NVIDIA CUDA toolkit and driver on the build machine.
For `llama-gpu-metal`, build on macOS with Xcode command-line tools installed.

## macOS Distribution

Build a native macOS app bundle and DMG installer on macOS:

```bash
bun run tauri:build:macos
```

The build script auto-downloads a compatible ONNX Runtime dylib for macOS into `src-tauri/onnxruntime` (unless `ORT_LIB_LOCATION` is explicitly set), and bundles it into the app resources.

Artifacts are generated under:

- `src-tauri/target/release/bundle/macos/*.app`
- `src-tauri/target/release/bundle/dmg/*.dmg`

## Contributing

We welcome contributions.

1. Fork the repo
2. Create a feature branch `git checkout -b feature/my-change`
3. Follow TypeScript and React best practices
4. Test your changes
5. Commit with clear, conventional messages
6. Push and open a PR

## License

GNU Affero General Public License v3.0 — see `LICENSE`

<div align="center">
  <p>Privacy-first • Local-first • Open Source</p>
</div>
