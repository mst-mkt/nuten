{ nutenOf }:
{
  lib,
  pkgs,
  config,
  mkFormatterModule,
  ...
}:
let
  cfg = config.programs.nuten;
  formatterModule = mkFormatterModule {
    name = "nuten";
    args = [ "format" ];
    includes = [
      "*.nu"
      "*.nuon"
    ];
  };
  configFile = pkgs.writeText "nuten.json" (builtins.toJSON cfg.settings);
in
{
  imports = [ formatterModule ];

  options.programs.nuten.settings = lib.mkOption {
    type = lib.types.submodule { freeformType = lib.types.json; };
    default = { };
    example = {
      line_width = 80;
    };
    description = ''
      Configuration to generate nuten.json with.
      When not empty, it is passed with `--config`, and config files in the project are ignored.
    '';
  };

  config = {
    programs.nuten.package = lib.mkDefault (nutenOf pkgs);
    settings.formatter.nuten.options = lib.mkIf (cfg.enable && cfg.settings != { }) [
      "--config"
      (toString configFile)
    ];
  };
}
