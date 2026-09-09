{
  description = "truss — release binaries, NixOS Wayland session, and development shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f:
        nixpkgs.lib.genAttrs systems (system: f (import nixpkgs { inherit system; }));
    in
    {
      packages = forAllSystems (pkgs: rec {
        truss = pkgs.callPackage ./nix/package.nix {
          release = builtins.fromJSON (builtins.readFile ./nix/release.json);
        };
        default = truss;
      });
      apps = forAllSystems (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/truss";
        };
        session = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/truss-session";
        };
      });
      nixosModules.truss = import ./nix/module.nix { inherit self; };
      nixosModules.default = self.nixosModules.truss;
      overlays.default = final: prev: {
        truss = final.callPackage ./nix/package.nix {
          release = builtins.fromJSON (builtins.readFile ./nix/release.json);
        };
      };
      checks = forAllSystems (pkgs: {
        release-package = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
        release-smoke = pkgs.runCommand "truss-release-smoke" {
          nativeBuildInputs = [ pkgs.python3 ];
        } ''
          python3 ${./tests/release_smoke.py} ${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/truss
          touch $out
        '';
        nixos-module = pkgs.writeText "truss-nixos-module-check.json" (builtins.toJSON (
          import ./tests/nixos-module.nix {
            inherit nixpkgs self;
            system = pkgs.stdenv.hostPlatform.system;
          }
        ));
        session-launcher = pkgs.runCommand "truss-session-launcher-tests" {
          nativeBuildInputs = [ pkgs.python3 ];
        } ''
          cp -r ${./tests/session_launcher.py} session_launcher.py
          mkdir -p resources tests
          cp ${./resources/truss-session} resources/truss-session
          cp session_launcher.py tests/session_launcher.py
          # The test uses explicit /bin/sh for non-Nix systems; patch for sandboxing.
          substituteInPlace tests/session_launcher.py --replace-fail /bin/sh ${pkgs.runtimeShell}
          python3 tests/session_launcher.py -v
          touch $out
        '';
      });
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            rustc
            cargo
            rustfmt
            clippy
            pkg-config
            wayland
            wayland-protocols
            libxkbcommon
            libinput
            mesa
            libgbm
            pixman
            udev
            seatd
            libx11
            libxcursor
            libxrandr
            libxi
            libglvnd
            vulkan-loader
            egl-wayland
          ];

          # Essential runtime environment for Intel, AMD, and NVIDIA GBM/EGL/Vulkan drivers
          LD_LIBRARY_PATH = with pkgs; lib.makeLibraryPath [
            wayland
            libxkbcommon
            libinput
            mesa
            libgbm
            pixman
            udev
            seatd
            libx11
            libxcursor
            libxrandr
            libxi
            libglvnd
            vulkan-loader
            egl-wayland
          ];

          # NVIDIA-specific Wayland / GBM fallback flags (compatible across Intel/AMD/Nvidia)
          GBM_BACKENDS_PATH = "/run/opengl-driver/lib/gbm:/run/current-system/sw/lib/gbm";
          __GLX_VENDOR_LIBRARY_NAME = "mesa";
        };
      });
    };
}
