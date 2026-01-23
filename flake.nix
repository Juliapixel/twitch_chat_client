{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-25.11";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ fenix.overlays.default ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.fenix.stable.toolchain
          ];

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (
            with pkgs;
            [
              expat
              fontconfig
              freetype
              freetype.dev
              libGL
              pkg-config
              xorg.libX11
              xorg.libXcursor
              xorg.libXi
              xorg.libXrandr
              wayland
              libxkbcommon
            ]
          );
        };

        packages.default = pkgs.rustPlatform.buildRustPackage (
          let
            manifest = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          in
          {
            pname = manifest.package.name;
            name = manifest.package.name;
            version = manifest.package.version;

            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
              allowBuiltinFetchGit = true;
            };

            meta = {
              description = "A native Twitch chat client written in Rust";
              license = pkgs.lib.licenses.mit;
            };
          }
        );
      }
    );
}
