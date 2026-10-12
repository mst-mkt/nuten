---
title: Helix
---

# Helix

Register `nuten lsp` as a language server in `languages.toml` and format on save. Helix uses the `nuten` on the PATH, so [install](/guide/installation) `nuten` first.

```toml [languages.toml]
[language-server.nuten]
command = "nuten"
args = ["lsp"]

[[language]]
name = "nu"
language-servers = ["nu-lsp", "nuten"]
auto-format = true
```

Since `language-servers` replaces Helix's default list, keep `nu-lsp`, the Nushell language server, in it. `nu-lsp` does not support formatting, so Nuten does the formatting.

To use a `nuten` that is not on the PATH, give its path in `command`.

## Nix

With home-manager, write the contents of `languages.toml` in `programs.helix.languages`.

```nix
{
  programs.helix = {
    enable = true;
    languages = {
      language-server.nuten = {
        command = "nuten";
        args = [ "lsp" ];
      };
      language = [
        {
          name = "nu";
          language-servers = [ "nu-lsp" "nuten" ];
          auto-format = true;
        }
      ];
    };
  };
}
```
