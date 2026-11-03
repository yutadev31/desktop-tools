{ pkgs, ... }:
(import ../build-package.nix { inherit pkgs; } "rubar").overrideAttrs {
  preCheck = ''
    export FONTCONFIG_FILE="${pkgs.makeFontsConf { fontDirectories = [ pkgs.dejavu_fonts ]; }}"
  '';
}
