{ pkgs }:
pname:
let
  lockfile = builtins.fromTOML (builtins.readFile ../Cargo.lock);
  package = builtins.head (builtins.filter (package: package.name == pname) lockfile.package);
in
pkgs.rustPlatform.buildRustPackage {
  inherit pname;
  inherit (package) version;
  src = ../.;
  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [
    "--package"
    pname
  ];
  cargoTestFlags = [
    "--package"
    pname
  ];
  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs = with pkgs; [
    libpulseaudio
    libxkbcommon
    wayland
  ];
}
