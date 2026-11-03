{ pkgs, ... }:
import ../build-package.nix { inherit pkgs; } "shot"
