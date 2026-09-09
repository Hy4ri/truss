{ self }:
{ config, lib, pkgs, ... }:
let cfg = config.programs.truss;
in {
  options.programs.truss = {
    enable = lib.mkEnableOption "Truss Wayland compositor";
    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "Truss release package, including its Wayland session.";
    };
    extraPackages = lib.mkOption {
      type = with lib.types; listOf package;
      default = with pkgs; [ kitty fuzzel waybar ];
      defaultText = lib.literalExpression "with pkgs; [ kitty fuzzel waybar ]";
      description = "Desktop tools used by the default Lua configuration.";
    };
  };
  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ] ++ cfg.extraPackages;
    services.displayManager.sessionPackages = [ cfg.package ];
    systemd.packages = [ cfg.package ];
    hardware.graphics.enable = lib.mkDefault true;
    services.dbus.enable = true;
    security.polkit.enable = true;
    systemd.user.services.truss-polkit-agent = {
      description = "Truss authentication agent";
      wantedBy = [ "truss-session.target" ];
      partOf = [ "truss-session.target" ];
      after = [ "graphical-session.target" ];
      serviceConfig = {
        ExecStart = "${pkgs.polkit_gnome}/libexec/polkit-gnome-authentication-agent-1";
        Restart = "on-failure";
        RestartSec = 2;
      };
    };
    xdg.portal = {
      enable = lib.mkDefault true;
      extraPortals = [ pkgs.xdg-desktop-portal-gtk ];
      config.truss.default = [ "gtk" ];
    };
    xdg.mime.enable = lib.mkDefault true;
    fonts.enableDefaultPackages = lib.mkDefault true;
    programs.dconf.enable = lib.mkDefault true;
    environment.sessionVariables.NIXOS_OZONE_WL = lib.mkDefault "1";
    # No automatic switch of display manager, autologin, or default session.
    # logind supplies seat/device permissions; no broad input/video groups.
  };
}
