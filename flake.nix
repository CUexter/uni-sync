{
  description = "Uni-Sync: Your project description";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { system = "x86_64-linux"; config.allowUnfree = true; };
      llvmPackages = pkgs.llvmPackages_14;
    in
    {
      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        pname = "uni-sync";
        version = "0.5.0";
        src = ./.;
        cargoLock = {
          lockFile = ./Cargo.lock;
        };

        nativeBuildInputs = with pkgs; [
          pkg-config
          rustc
          cargo
          llvmPackages.libclang
          llvmPackages.clang
        ];

        buildInputs = with pkgs; [
          udev
          libudev-zero
          systemd.dev # This provides libudev.pc
          lm_sensors
          glibc.dev
          gcc
          linuxPackages.nvidia_x11
        ];

        LIBCLANG_PATH = "${llvmPackages.libclang.lib}/lib";

        RUSTFLAGS = "-C link-arg=-lsensors -C link-arg=-ludev";

        preBuild = ''
          export LD_LIBRARY_PATH="${with pkgs; lib.makeLibraryPath [
            udev
            systemd.dev
            lm_sensors
            llvmPackages.libclang
            glibc.dev
            gcc.cc.lib
            linuxPackages.nvidia_x11
          ]}"
          export PKG_CONFIG_PATH="${pkgs.pkg-config}/lib/pkgconfig:${pkgs.lm_sensors}/lib/pkgconfig:${pkgs.systemd.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
          export SENSORS_LIB_DIR="${pkgs.lm_sensors}/lib"
          export SENSORS_INCLUDE_DIR="${pkgs.lm_sensors}/include"
          export BINDGEN_EXTRA_CLANG_ARGS="-I${llvmPackages.libclang.lib}/lib/clang/${llvmPackages.libclang.version}/include -I${pkgs.glibc.dev}/include -I${pkgs.gcc}/lib/gcc/${pkgs.stdenv.targetPlatform.config}/${pkgs.gcc.version}/include -I${pkgs.gcc}/lib/gcc/${pkgs.stdenv.targetPlatform.config}/${pkgs.gcc.version}/include-fixed -I${pkgs.systemd.dev}/include"
          export CPATH="${with pkgs; lib.makeSearchPathOutput "dev" "include" [
            glibc.dev
            gcc.cc
            systemd.dev
          ]}"
          export LIBRARY_PATH="${pkgs.lib.makeLibraryPath [ pkgs.lm_sensors pkgs.systemd.dev pkgs.linuxPackages.nvidia_x11]}"
        '';

        postInstall = ''
          patchelf --set-rpath "${pkgs.lm_sensors}/lib:${pkgs.systemd.dev}/lib:${pkgs.udev}/lib:${pkgs.linuxPackages.nvidia_x11}/lib:$out/lib" $out/bin/uni-sync
        '';

        meta = with pkgs.lib; {
          description = "Uni-sync with fan curves";
          homepage = "https://github.com/CUexter/uni-sync";
          license = licenses.mit;
          maintainers = [ maintainers.cuexter ];
        };
      };

      devShells.${system}.default = pkgs.mkShell {
        inputsFrom = [ self.packages.${system}.default ];
        packages = with pkgs; [
          rustc
          cargo
          rust-analyzer
        ];
        shellHook = ''
          export LIBCLANG_PATH="${llvmPackages.libclang.lib}/lib"
          export LD_LIBRARY_PATH="${with pkgs; lib.makeLibraryPath [
            udev
            systemd.dev
            lm_sensors
            llvmPackages.libclang
            glibc.dev
            gcc.cc.lib
            linuxPackages.nvidia_x11
          ]}"
          export PKG_CONFIG_PATH="${pkgs.pkg-config}/lib/pkgconfig:${pkgs.lm_sensors}/lib/pkgconfig:${pkgs.systemd.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
          export SENSORS_LIB_DIR="${pkgs.lm_sensors}/lib"
          export SENSORS_INCLUDE_DIR="${pkgs.lm_sensors}/include"
          export BINDGEN_EXTRA_CLANG_ARGS="-I${llvmPackages.libclang.lib}/lib/clang/${llvmPackages.libclang.version}/include -I${pkgs.glibc.dev}/include -I${pkgs.gcc}/lib/gcc/${pkgs.stdenv.targetPlatform.config}/${pkgs.gcc.version}/include -I${pkgs.gcc}/lib/gcc/${pkgs.stdenv.targetPlatform.config}/${pkgs.gcc.version}/include-fixed -I${pkgs.systemd.dev}/include"
          export CPATH="${with pkgs; lib.makeSearchPathOutput "dev" "include" [
            glibc.dev
            gcc.cc
            systemd.dev
          ]}"
          export LIBRARY_PATH="${pkgs.lib.makeLibraryPath [ pkgs.lm_sensors pkgs.systemd.dev ]}"
          export RUSTFLAGS="-C link-arg=-lsensors -C link-arg=-ludev"
        '';
      };

      nixosModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.services.uni-sync;
        in
        {
          options.services.uni-sync = {
            enable = lib.mkEnableOption "Uni-Sync service";
            configFile = lib.mkOption {
              type = lib.types.path;
              default = "/etc/uni-sync/uni-sync.json";
              description = "Path to the uni-sync configuration file";
            };
          };

          config = lib.mkIf cfg.enable {
            systemd.services.uni-sync = {
              description = "Uni-Sync Service";
              after = [ "network.target" ];
              wantedBy = [ "multi-user.target" ];
              serviceConfig = {
                ExecStartPre = pkgs.writeScript "uni-sync-init" ''
                  #!${pkgs.stdenv.shell}
                  mkdir -p $(dirname ${cfg.configFile})
                '';
                ExecStart = "${self.packages.${pkgs.system}.default}/bin/uni-sync --config ${cfg.configFile}";
                Restart = "always";
                User = "root";
              };
            };
          };
        };
    };
}
