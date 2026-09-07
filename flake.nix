{
  description = "Shed dotfile synchronization CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, fenix }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

      packageFor = system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          toolchain = fenix.packages.${system}.fromToolchainFile {
            file = ./rust-toolchain.toml;
            sha256 = "sha256-A1abGIbOtcBSdrUMhDGrER3pRM1hQP4fp9gh3Y4PKc8=";
          };
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
        in
        rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          version = cargoToml.package.version;
          src = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./src
            ];
          };
          cargoLock.lockFile = ./Cargo.lock;
          strictDeps = true;
          meta = {
            inherit (cargoToml.package) description homepage;
            license = pkgs.lib.licenses.mit;
            mainProgram = "shed";
            platforms = systems;
          };
        };
    in
    {
      packages = forAllSystems (system: {
        default = packageFor system;
      });

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/shed";
          meta.description = cargoToml.package.description;
        };
      });

      checks = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          package = self.packages.${system}.default;
        in
        {
          default = pkgs.runCommand "shed-smoke-test" {
            nativeBuildInputs = [ package ];
          } ''
            test "$(shed --version)" = "shed ${cargoToml.package.version}"
            mkdir "$out"
          '';
        });
    };
}
