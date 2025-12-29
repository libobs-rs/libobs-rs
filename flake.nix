{
  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixos-unstable/nixexprs.tar.xz";

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake {inherit inputs;} {
      perSystem = {
        lib,
        pkgs,
        system,
        ...
      }: {
        _module.args.pkgs = with inputs;
          import nixpkgs {
            inherit system;
            overlays = [(import rust-overlay)];
          };

        # No Nix package output needed; we use devShell for local development.

        # Development shell for running examples via `nix develop`
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            clang
            openssl
            pkg-config
            # include rust-src in the toolchain
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" ];
            })
            obs-studio
            simde
            glib
            xorg.libX11
            wayland
            wayland-protocols
            egl-wayland
            libxkbcommon
            libglvnd
            mesa
            libxcb-util
            cargo-hack
            cargo-nextest
            ffmpeg
          ];

          shellHook = ''
            # Ensure libEGL/libGL can locate vendor and drivers under Nix store
            export LIBGL_DRIVERS_PATH=${pkgs.mesa}/lib/dri
            export GBM_DRIVERS_PATH=${pkgs.mesa}/lib/dri

            # Bindgen/libclang path so headers can be parsed properly
            export LIBCLANG_PATH=${pkgs.llvmPackages.libclang.lib}/lib

            # Make GLVND vendor JSON discoverable for libEGL/libGL
            if [ -n "$XDG_DATA_DIRS" ]; then
              export XDG_DATA_DIRS=${pkgs.libglvnd}/share:${pkgs.wayland}/share:$XDG_DATA_DIRS
            else
              export XDG_DATA_DIRS=${pkgs.libglvnd}/share:${pkgs.wayland}/share
            fi

            # Help dynamic linker find core libs in the dev shell
            if [ -n "$LD_LIBRARY_PATH" ]; then
              export LD_LIBRARY_PATH=${pkgs.libglvnd}/lib:${pkgs.mesa}/lib:${pkgs.wayland}/lib:${pkgs.libxkbcommon}/lib:${pkgs.obs-studio}/lib:${pkgs.xorg.libX11}/lib:${pkgs.glib}/lib:${pkgs.simde}/lib:${pkgs.stdenv.cc.cc.lib}/lib:$LD_LIBRARY_PATH
            else
              export LD_LIBRARY_PATH=${pkgs.libglvnd}/lib:${pkgs.mesa}/lib:${pkgs.wayland}/lib:${pkgs.libxkbcommon}/lib:${pkgs.obs-studio}/lib:${pkgs.xorg.libX11}/lib:${pkgs.glib}/lib:${pkgs.simde}/lib:${pkgs.stdenv.cc.cc.lib}/lib
            fi
          '';
        };
      };

      systems = inputs.nixpkgs.lib.systems.flakeExposed;
    };
}