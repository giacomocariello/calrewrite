{
  description = "calrewrite - iCal timeshift proxy server";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      ...
    }:
    let
      perSystem = flake-utils.lib.eachDefaultSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};

          inherit (pkgs) lib;

          craneLib = crane.mkLib pkgs;
          src = craneLib.cleanCargoSource ./.;

          commonArgs = {
            inherit src;
            strictDeps = true;

            buildInputs =
              [
                # No native TLS deps needed — reqwest uses rustls-tls
              ]
              ++ lib.optionals pkgs.stdenv.isDarwin [
                pkgs.libiconv
              ];
          };

          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          calrewrite = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
              meta.mainProgram = "calrewrite";
            }
          );
        in
        {
          checks = {
            inherit calrewrite;

            calrewrite-clippy = craneLib.cargoClippy (
              commonArgs
              // {
                inherit cargoArtifacts;
                cargoClippyExtraArgs = "--all-targets -- --deny warnings";
              }
            );

            calrewrite-fmt = craneLib.cargoFmt {
              inherit src;
            };
          };

          packages = {
            default = calrewrite;
          };

          apps.default = flake-utils.lib.mkApp {
            drv = calrewrite;
          };

          devShells.default = craneLib.devShell {
            checks = self.checks.${system};

            packages = [
              pkgs.cargo-watch
            ];
          };
        }
      );
    in
    perSystem
    // {
      nixosModules.default = self.nixosModules.calrewrite;

      nixosModules.calrewrite =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          cfg = config.services.calrewrite;
        in
        {
          options.services.calrewrite = {
            enable = lib.mkEnableOption "calrewrite iCal timeshift proxy";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
              defaultText = lib.literalExpression "inputs.calrewrite.packages.\${pkgs.stdenv.hostPlatform.system}.default";
              description = "The calrewrite package to use.";
            };

            host = lib.mkOption {
              type = lib.types.str;
              default = "127.0.0.1";
              description = "Address to bind to.";
            };

            port = lib.mkOption {
              type = lib.types.port;
              default = 3000;
              description = "Port to listen on.";
            };

            openFirewall = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = "Whether to open the firewall for the configured port.";
            };
          };

          config = lib.mkIf cfg.enable {
            systemd.services.calrewrite = {
              description = "calrewrite iCal timeshift proxy";
              wantedBy = [ "multi-user.target" ];
              after = [ "network.target" ];

              environment = {
                CALREWRITE_HOST = cfg.host;
                CALREWRITE_PORT = toString cfg.port;
              };

              serviceConfig = {
                ExecStart = lib.getExe cfg.package;
                Restart = "on-failure";
                RestartSec = 5;

                DynamicUser = true;
                NoNewPrivileges = true;
                ProtectSystem = "strict";
                ProtectHome = true;
                PrivateTmp = true;
                PrivateDevices = true;
                ProtectKernelTunables = true;
                ProtectKernelModules = true;
                ProtectControlGroups = true;
                RestrictSUIDSGID = true;
                RestrictNamespaces = true;
                LockPersonality = true;
                MemoryDenyWriteExecute = true;
                RestrictRealtime = true;
                SystemCallFilter = [ "@system-service" ];
                SystemCallArchitectures = "native";
              };
            };

            networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.port ];
          };
        };
    };
}
