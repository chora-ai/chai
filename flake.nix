{
  description = "chai – multi-agent management system";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  };

  outputs = { self, nixpkgs }:
  let
    systems = [
      "x86_64-linux"
      "aarch64-linux"
      "aarch64-darwin"
    ];

    forEachSystem = fn: nixpkgs.lib.genAttrs systems (system: fn system);

    pkgsFor = system: import nixpkgs { inherit system; };

    # Native build inputs shared by all package targets.
    commonNativeBuildInputs = pkgs: with pkgs; [
      pkg-config
    ];

    # Build inputs shared by all package targets.
    commonBuildInputs = pkgs: with pkgs; [
      openssl
    ];

    # Build inputs for GUI targets (desktop). Only needed on Linux.
    guiBuildInputs = pkgs: with pkgs; [
      libxcursor
      libxi
      libxrandr
    ];
  in
  {
    # Shells for `nix develop`
    devShells = forEachSystem (system:
      let pkgs = pkgsFor system;
      in
      {
        default = pkgs.mkShell {
          nativeBuildInputs = commonNativeBuildInputs pkgs ++ [ pkgs.cargo ];
          buildInputs = commonBuildInputs pkgs
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux (guiBuildInputs pkgs);
          shellHook = pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            export LD_LIBRARY_PATH="${pkgs.libGL}/lib"
          '';
        };
      }
    );

    # Packages for `nix build`
    packages = forEachSystem (system:
      let pkgs = pkgsFor system;
      in
      {
        # All default-members from Cargo.toml (cli + desktop)
        default = pkgs.rustPlatform.buildRustPackage {
          name = "chai";
          src = ./.;
          nativeBuildInputs = commonNativeBuildInputs pkgs;
          buildInputs = commonBuildInputs pkgs
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux (guiBuildInputs pkgs);
          cargoLock.lockFile = ./Cargo.lock;
        };

        # crates/cli → chai binary
        cli = pkgs.rustPlatform.buildRustPackage {
          name = "chai-cli";
          src = ./.;
          nativeBuildInputs = commonNativeBuildInputs pkgs;
          buildInputs = commonBuildInputs pkgs;
          cargoBuildFlags = [ "--manifest-path crates/cli/Cargo.toml" ];
          cargoLock.lockFile = ./Cargo.lock;
        };

        # crates/desktop → chai-desktop binary
        desktop = pkgs.rustPlatform.buildRustPackage {
          name = "chai-desktop";
          src = ./.;
          nativeBuildInputs = commonNativeBuildInputs pkgs;
          buildInputs = commonBuildInputs pkgs
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux (guiBuildInputs pkgs);
          cargoBuildFlags = [ "--manifest-path crates/desktop/Cargo.toml" ];
          cargoLock.lockFile = ./Cargo.lock;
        };
      }
    );
  };
}
