{
  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixos-26.05/nixexprs.tar.zst";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      crane,
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
        ld_library_path = pkgs.lib.makeLibraryPath (
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

        craneLib = crane.mkLib pkgs;

        commonArgs = {
          src = ./.;
          strictDeps = true;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        manifest = builtins.fromTOML (builtins.readFile ./Cargo.toml);

        juliarino =
          (
            craneLib.buildPackage commonArgs
            // {
              inherit cargoArtifacts;

              meta = {
                description = "A native Twitch chat client written in Rust";
                license = pkgs.lib.licenses.mit;
              };
            }
          ).overrideAttrs
            (
              final: prev: {
                nativeBuildInputs =
                  prev.nativeBuildInputs
                  ++ (with pkgs; [
                    makeWrapper
                  ]);
                postInstall = ''
                  wrapProgram $out/bin/${manifest.package.name} --set LD_LIBRARY_PATH ${ld_library_path}
                '';
              }
            );
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.fenix.stable.toolchain
          ];

          LD_LIBRARY_PATH = ld_library_path;
        };

        packages.default = juliarino;
      }
    );
}
