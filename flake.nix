{
  description = "truss — minimal Wayland compositor (smithay) dev shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f:
        nixpkgs.lib.genAttrs systems (system: f (import nixpkgs { inherit system; }));
    in
    {
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
