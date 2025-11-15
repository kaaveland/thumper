{
  description = "Syncing files to BunnyCDN";
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk.url = "github:nix-community/naersk";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs =
    {
      fenix,
      naersk,
      nixpkgs,
      flake-utils,
      ...
    }:
    let
      overlay = final: prev: {

        thumper =
          let
            inherit (final.stdenv.hostPlatform) system;

            targets = {
              x86_64-linux = fenix.packages.${system}.targets.x86_64-unknown-linux-gnu;
              aarch64-linux = fenix.packages.${system}.targets.aarch64-unknown-linux-gnu;
              aarch64-darwin = fenix.packages.${system}.targets.aarch64-apple-darwin;
            };

            target =
              targets.${system} or (builtins.throw (
                "Unsupported system: "
                + system
                + ". Supported systems are: "
                + builtins.concatStringsSep ", " (builtins.attrNames targets)
              ));

            toolchain =
              with fenix.packages.${system};
              combine [
                target.stable.rust-std
                stable.rust-src
                stable.rustc
                stable.cargo
                stable.clippy
                stable.rustfmt
              ];

            naersk' = final.callPackage naersk {
              cargo = toolchain;
              rustc = toolchain;
            };
          in
          naersk'.buildPackage {
            src = ./.;

            meta = {
              description = "A tool to sync files to BunnyCDN";
              license = with final.lib.licenses; [ mit ];
              maintainers = with final.lib.maintainers; [ stv0g ];
              platforms = final.lib.platforms.all;
              mainProgram = "thumper";
            };
          };
      };
    in
    {
      overlays.default = overlay;
    }
    // flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ overlay ];
        };
      in
      {
        packages.default = pkgs.thumper;

        devShells.default = pkgs.mkShell {
          inputsFrom = [ pkgs.thumper ];
        };
      }
    );
}
