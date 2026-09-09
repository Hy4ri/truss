{ lib, stdenv, fetchurl, autoPatchelfHook, makeWrapper, dbus, systemd,
  wayland, libxkbcommon, libinput, libgbm, udev, seatd, libglvnd,
  libx11, libxcursor, libxrandr, libxi, xkeyboard_config, coreutils,
  release }:
stdenv.mkDerivation {
  pname = "truss";
  version = release.version;
  src = fetchurl {
    url = "https://github.com/Hy4ri/truss/releases/download/v${release.version}/truss-${release.version}-${stdenv.hostPlatform.system}.tar.gz";
    hash = release.hashes.${stdenv.hostPlatform.system};
  };
  nativeBuildInputs = [ autoPatchelfHook makeWrapper ];
  buildInputs = [ stdenv.cc.cc.lib wayland libxkbcommon libinput libgbm udev seatd libglvnd ];
  # Winit/EGL load these dynamically; DT_NEEDED alone is insufficient.
  runtimeDependencies = map lib.getLib [ wayland libxkbcommon libglvnd libx11 libxcursor libxrandr libxi ];
  sourceRoot = ".";
  dontConfigure = true;
  dontBuild = true;
  installPhase = ''
    runHook preInstall
    mkdir -p $out
    cp -r bin share lib $out/
    mkdir -p $out/share/truss
    cp -r examples $out/share/truss/examples
    substituteInPlace $out/share/wayland-sessions/truss.desktop \
      --replace-fail 'TryExec=truss-session' "TryExec=$out/bin/truss-session" \
      --replace-fail 'Exec=truss-session' "Exec=$out/bin/truss-session"
    runHook postInstall
  '';
  postFixup = ''
    wrapProgram $out/bin/truss \
      --set-default XKB_CONFIG_ROOT ${xkeyboard_config}/share/X11/xkb \
      --prefix LD_LIBRARY_PATH : /run/opengl-driver/lib
    wrapProgram $out/bin/truss-session \
      --prefix PATH : ${lib.makeBinPath [ dbus systemd coreutils ]}:$out/bin
  '';
  passthru.providedSessions = [ "truss" ];
  meta = {
    description = "Keyboard-first dynamic tiling Wayland compositor";
    homepage = "https://github.com/Hy4ri/truss";
    platforms = [ "x86_64-linux" "aarch64-linux" ];
    mainProgram = "truss";
  };
}
