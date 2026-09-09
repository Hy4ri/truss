# Evaluated by flake checks; no host rebuild or activation is performed.
{ nixpkgs, self, system }:
let
  eval = modules: import (nixpkgs + "/nixos/lib/eval-config.nix") {
    inherit system;
    modules = [ self.nixosModules.default { system.stateVersion = "26.05"; } ] ++ modules;
  };
  enabled = (eval [{ programs.truss.enable = true; }]).config;
  disabled = (eval []).config;
  package = self.packages.${system}.default;
  lib = nixpkgs.lib;
in
assert lib.elem package enabled.environment.systemPackages;
assert lib.elem package enabled.services.displayManager.sessionPackages;
assert lib.elem package enabled.systemd.packages;
assert enabled.hardware.graphics.enable;
assert enabled.services.dbus.enable;
assert enabled.security.polkit.enable;
assert enabled.xdg.portal.enable;
assert enabled.xdg.portal.config.truss.default == "gtk";
assert enabled.systemd.user.services.truss-polkit-agent.partOf == [ "truss-session.target" ];
assert !lib.elem package disabled.services.displayManager.sessionPackages;
assert !enabled.services.displayManager.sddm.enable;
{
  inherit system;
  package = package.name;
  providedSessions = package.providedSessions;
  sessionPackages = map (p: p.name) enabled.services.displayManager.sessionPackages;
  polkitAgent = enabled.systemd.user.services.truss-polkit-agent.serviceConfig.ExecStart;
  portal = enabled.xdg.portal.config.truss.default;
  defaultDesktopTools = map (p: p.pname) enabled.programs.truss.extraPackages;
  disabledModuleDoesNotRegisterSession = true;
}
