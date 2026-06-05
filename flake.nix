{
  description = "flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  };

  outputs = { self, nixpkgs }:
  let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
  in
  {
    # shells for `nix develop`
    devShells.${system} = {

      # default shell
      default = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [
          cargo
          pkg-config
        ];
        buildInputs = with pkgs; [
          openssl
          libxcursor
          libxi
          libxrandr
        ];
        shellHook = ''
          export LD_LIBRARY_PATH="${pkgs.libGL}/lib"
        '';
      };
    };

    # packages for `nix build`
    packages.${system} = {

      # all binaries
      default = pkgs.rustPlatform.buildRustPackage {
        name = "chai-binaries";
        src = ./.;
        nativeBuildInputs = with pkgs; [
          pkg-config
        ];
        buildInputs = with pkgs; [
          openssl
          libxcursor
          libxi
          libxrandr
        ];
        cargoBuildFlags = [
          #"--features=matrix"
        ];
        cargoLock.lockFile = ./Cargo.lock;
      };

      # crates/cli (chai)
      cli = pkgs.rustPlatform.buildRustPackage {
        name = "chai";
        src = ./.;
        nativeBuildInputs = with pkgs; [
          pkg-config
        ];
        buildInputs = with pkgs; [
          openssl
        ];
        cargoBuildFlags = [
          #"--features=matrix"
          "--manifest-path crates/cli/Cargo.toml"
        ];
        cargoLock.lockFile = ./Cargo.lock;
      };

      # crates/desktop (chai-desktop)
      desktop = pkgs.rustPlatform.buildRustPackage {
        name = "chai-desktop";
        src = ./.;
        nativeBuildInputs = with pkgs; [
          pkg-config
        ];
        buildInputs = with pkgs; [
          openssl
          libxcursor
          libxi
          libxrandr
        ];
        cargoBuildFlags = [
          #"--features=matrix"
          "--manifest-path crates/desktop/Cargo.toml"
        ];
        cargoLock.lockFile = ./Cargo.lock;
      };
    };
  };
}
