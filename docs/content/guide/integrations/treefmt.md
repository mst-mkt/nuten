---
title: treefmt
---

# treefmt

## treefmt-nix

Import `treefmtModule` to enable the `programs.nuten` option in treefmt-nix.

```nix
treefmt-nix.lib.evalModule pkgs {
  imports = [ inputs.nuten.treefmtModule ]; # github:mst-mkt/nuten
  programs.nuten = {
    enable = true;
    excludes = [ "tests/fixtures/*" ];
    settings = {
      line_width = 80;
    };
  };
}
```

`settings` accepts the same options as the [config file](/guide/configuration). Write `include` and `exclude` in treefmt's `includes` and `excludes`, not in `settings`.

When `settings` is set, Nuten ignores the project's config file. If you also run Nuten outside treefmt, such as with `nuten format` or from an editor, leave out `settings` and keep your settings in the project's [config file](/guide/configuration). In that case, list the files to format in `includes` and `excludes` of `programs.nuten`, and also in `include` and `exclude` of the config file.

Prebuilt binaries are available from the [binary cache](/guide/installation#binary-cache).

## treefmt.toml

treefmt uses the `nuten` on the PATH, so [install](/guide/installation) `nuten` first.

```toml [treefmt.toml]
[formatter.nuten]
command = "nuten"
options = ["format"]
includes = ["*.nu", "*.nuon"]
```

Formatting settings go in the project's [config file](/guide/configuration).

Specify the files to format in `includes` and `excludes` of `[formatter.nuten]`. If you also run Nuten outside treefmt, such as with `nuten format` or from an editor, repeat the same patterns in `include` and `exclude` of the config file.
