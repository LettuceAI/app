{
  description = "LettuceAI project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    bun2nix.url = "github:baileyluTCD/bun2nix";
  };

  outputs = { self, nixpkgs, flake-utils, bun2nix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ bun2nix.overlays.default ];
        };

        cleanSrc = builtins.path {
          path = ./.;
          name = "lettuceai-src";
          filter = path: type:
            let
              baseName = pkgs.lib.baseNameOf path;
            in
              baseName != "target" && baseName != ".git";
        };

        src = pkgs.runCommandLocal "lettuceai-src" { } ''
          mkdir -p "$out"
          cp -r --no-preserve=mode,ownership,timestamps ${cleanSrc}/. "$out"/
          ln -s src-tauri/Cargo.lock "$out"/Cargo.lock
        '';

        runtimeLibs = with pkgs; [
          gtk3
          webkitgtk_4_1
          libsoup_3
          libayatana-appindicator
          librsvg
          openssl
          glib
          gdk-pixbuf
          cairo
          pango
          atk
          dbus
          fontconfig
          freetype
          libglvnd
          libx11
          libxcursor
          libxi
          libxrandr
          libxdamage
          libxfixes
          libxcomposite
          libxext
          libxinerama
          libxtst
          libxcb
          libxkbfile
          gsettings-desktop-schemas
          stdenv.cc.cc.lib
        ];

        runtimePackages = with pkgs; [
          espeak-ng
        ];

        runtimeGstreamerPackages = with pkgs.gst_all_1; [
          gstreamer.out
          gst-plugins-base
          gst-plugins-good
          gst-plugins-bad
          gst-libav
        ];

        toolchain = with pkgs; [
          bun
          clang
          cmake
          nodejs_22
          cargo
          rustc
          rustfmt
          clippy
          libclang
          pkg-config
        ];

        ldLibraryPath = pkgs.lib.makeLibraryPath (runtimeLibs ++ runtimeGstreamerPackages);
        libclangPath = "${pkgs.lib.getLib pkgs.libclang}/lib";
        onnxruntimeLibPath = "${pkgs.onnxruntime}/lib/libonnxruntime.so";
        gstPluginPath = pkgs.lib.makeSearchPath "lib/gstreamer-1.0" runtimeGstreamerPackages;
        bunNix = pkgs.runCommandLocal "lettuceai-bun-nix" {
          nativeBuildInputs = [ pkgs.bun2nix ];
        } ''
          mkdir -p "$out"
          ${pkgs.bun2nix}/bin/bun2nix -l ${src}/bun.lock > "$out/bun.nix"
        '';

        lettuceai = pkgs.rustPlatform.buildRustPackage {
          pname = "lettuceai";
          version = "1.2.0";
          src = src;
          doCheck = false;
          cargoLock = {
            lockFile = ./src-tauri/Cargo.lock;
            outputHashes = {
              "whisper-rs-0.16.0" = "sha256-0Lnd4zATtWAJkDjXGrH7tLA+tXlS1FgZmUCOWFitjLE=";
              "whisper-rs-sys-0.15.0" = "sha256-0Lnd4zATtWAJkDjXGrH7tLA+tXlS1FgZmUCOWFitjLE=";
            };
          };
          cargoHash = pkgs.lib.fakeHash;

          nativeBuildInputs = with pkgs; [
            pkgs.bun2nix.hook
            bun
            clang
            cmake
            nodejs_22
            libclang
            pkg-config
            makeWrapper
            wrapGAppsHook3
          ];

          buildInputs = runtimeLibs ++ runtimeGstreamerPackages ++ [
            pkgs.onnxruntime
            pkgs.espeak-ng
          ];

          ORT_LIB_LOCATION = "${pkgs.onnxruntime}/lib";
          LIBCLANG_PATH = libclangPath;
          bunDeps = pkgs.bun2nix.fetchBunDeps {
            src = src;
            bunNix = "${bunNix}/bun.nix";
          };

          buildPhase = ''
            runHook preBuild

            bun run tauri build --no-bundle

            runHook postBuild
          '';

          installPhase = ''
            runHook preInstall

            install -Dm755 src-tauri/target/release/lettuceai $out/bin/lettuceai

            runHook postInstall
          '';

          postFixup = ''
            wrapProgram $out/bin/lettuceai \
              --set LD_LIBRARY_PATH "${ldLibraryPath}" \
              --set ORT_DYLIB_PATH "${onnxruntimeLibPath}" \
              --set GST_PLUGIN_SYSTEM_PATH_1_0 "${gstPluginPath}" \
              --set GST_PLUGIN_PATH_1_0 "${gstPluginPath}" \
              --prefix PATH : "${pkgs.lib.makeBinPath runtimePackages}"
          '';

          meta = {
            mainProgram = "lettuceai";
          };
        };
      in {
        apps.lettuce = flake-utils.lib.mkApp {
          drv = lettuceai;
        };

        apps.default = self.apps.${system}.lettuce;

        packages.default = lettuceai;

        devShells.default = pkgs.mkShell {
          packages = toolchain ++ runtimePackages ++ runtimeGstreamerPackages;

          buildInputs = runtimeLibs ++ runtimeGstreamerPackages;

          shellHook = ''
            export LD_LIBRARY_PATH="${ldLibraryPath}:$LD_LIBRARY_PATH"
            export LIBCLANG_PATH="${libclangPath}"
            export ORT_DYLIB_PATH="${onnxruntimeLibPath}"
            export GST_PLUGIN_SYSTEM_PATH_1_0="${gstPluginPath}"
            export GST_PLUGIN_PATH_1_0="${gstPluginPath}"
            export PATH="${pkgs.lib.makeBinPath runtimePackages}:$PATH"
          '';
        };
      });
}
