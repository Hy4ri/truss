# Install Truss

## NixOS (recommended)

Add the input to your system flake and import the module into the host's modules:

```nix
{
  inputs.truss.url = "github:Hy4ri/truss/feat/release-nix-session";

  outputs = { nixpkgs, truss, ... }: {
    nixosConfigurations.my-host = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux"; # also aarch64-linux
      modules = [
        ./configuration.nix
        truss.nixosModules.default
        { programs.truss.enable = true; }
      ];
    };
  };
}
```

The branch URL is intentional while the packaging PR awaits merge. Change it
back to `github:Hy4ri/truss` after that PR lands, then update your input lock.

Run your usual `sudo nixos-rebuild switch --flake .#my-host`, log out, and select
**Truss** in your existing display manager. Alternatively, log in on an active
local TTY and run `truss-session` (not sudo, not from SSH).

The module installs the release binary and registers
`share/wayland-sessions/truss.desktop`, enables graphics, D-Bus, polkit, GTK
portals, default fonts and dconf, and supplies Kitty, Fuzzel and Waybar.
`programs.truss.extraPackages` replaces that optional desktop-tools list;
`programs.truss.package` overrides the compositor package.

It does **not** choose/replace your display manager, enable autologin, set a
default session, change GPU-specific driver settings, or add broad input/video
permissions. NixOS logind grants devices to the active local login. Keep your
working Intel/AMD/NVIDIA configuration. No Mesa-vendor override is forced.

The session launcher selects the TTY backend, sets the Wayland desktop variables,
uses or creates a session D-Bus, imports activation variables, and starts the
Truss graphical-session target when user systemd is available. Logout stops the
target and clears imported desktop/display variables. The NixOS module ties its
polkit authentication agent to this target.

GTK portals provide supported desktop operations such as file picking. This does
not add compositor protocols: **screen sharing, a secure session locker, and
XWayland are not promised by this package**. Audio (for example PipeWire), a
notification daemon, network applets and power management remain host choices.

## Package-only / nested testing

```sh
nix run github:Hy4ri/truss/feat/release-nix-session -- --version
nix profile add github:Hy4ri/truss/feat/release-nix-session
truss --backend winit
```

A profile install alone does **not** register a display-manager session or enable
system services. Use the NixOS module for that. Native graphics on non-NixOS need
host-driver integration (such as nixGL); a successful `--version` is not a GPU test.

The default package downloads a versioned GitHub Release tarball with a pinned
SHA-256 from `nix/release.json`, patches the ELF interpreter/library paths and
wraps runtime dependencies. It does **not compile Rust**. `nix develop` remains
the source-development shell, separate from the installed release.

## First login

```sh
truss init-config
```

This creates `~/.config/truss/config.lua` and refuses to overwrite an existing
configuration. Review the example monitor names/modes for your hardware. The
embedded defaults also work without creating this file. `Super+Return` opens
Kitty, `Super+d` opens Fuzzel, and `Super+Shift+e` exits the session. A custom Lua
config replaces, rather than extends, embedded settings and keybindings.

Waybar starts from the default Lua config. The archive includes optional example
Waybar configuration under `share/truss/examples/waybar`; copy/adapt it if you
want the Truss workspace module (its Python script also needs Python 3). No user
configuration is overwritten during installation.

## Native Linux release archive

Download the tarball matching your architecture from
[GitHub Releases](https://github.com/Hy4ri/truss/releases) and verify it against
the supplied SHA-256 file. These are dynamically linked Ubuntu 24.04 builds
(glibc 2.39 baseline), not universal static executables. Your distribution must
provide Wayland, EGL/GBM, xkbcommon, libinput, libudev, libseat and their
transitive dependencies. Inspect `ldd bin/truss` before installation.

Install `bin/truss` and `bin/truss-session` into a system PATH directory, the
`.desktop` file into `/usr/share/wayland-sessions/`, and optionally the user
systemd target into `/usr/lib/systemd/user/`. Supply D-Bus tools and logind (or
explicitly configure a working seatd backend), a polkit agent, fonts, terminal
and launcher. On NixOS, use the flake instead of copying unpatched ELF files.

## Release maintenance

`scripts/build-release.sh` creates versioned architecture archives and checksums.
The release workflow builds/tests both architectures on native runners. Tag
builds publish only after their gates pass. The initial release is published from
green PR build artifacts; its tag identifies the exact packaging-build commit.

For a later release, update the Cargo version/lockfile, review and test the change,
publish the matching `vVERSION` tag, then set `nix/release.json` to the version and
**actual downloaded archive** SRI hashes. Do not overwrite existing release
assets or use a moving `latest` URL: the hash is an integrity pin. The flake may
point to the previous good release until the new assets exist. Test both
`nix flake check` and `nix build .#default`, execute `--version`, and smoke-test
headless IPC. A real TTY/graphics login is a separate host acceptance gate.
