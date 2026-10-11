---
title: Neovim
---

# Neovim

Set up Nuten with one of Neovim's built-in vim.lsp, the conform.nvim plugin, or the coc.nvim plugin. All methods use the `nuten` on the PATH, so [install](/guide/installation) `nuten` first. To use a `nuten` that is not on the PATH, give its path in `cmd` or `command`.

Neovim detects `.nu` as the `nu` filetype, but not `.nuon`. To format `.nuon` files too, assign them the `nu` filetype.

```lua
vim.filetype.add({ extension = { nuon = "nu" } })
```

## vim.lsp

Register `nuten lsp` as a language server and format on save. Neovim 0.11 or later is required.

```lua
vim.lsp.config("nuten", {
  cmd = { "nuten", "lsp" },
  filetypes = { "nu" },
  on_attach = function(client, bufnr)
    vim.api.nvim_create_autocmd("BufWritePre", {
      group = vim.api.nvim_create_augroup("nuten-format-" .. bufnr, {}),
      buffer = bufnr,
      callback = function()
        vim.lsp.buf.format({ bufnr = bufnr, id = client.id })
      end,
    })
  end,
})
vim.lsp.enable("nuten")
```

If you use [lsp-format.nvim](https://github.com/lukas-reineke/lsp-format.nvim), let its `on_attach` handle formatting on save.

```lua
require("lsp-format").setup()
vim.lsp.config("nuten", {
  cmd = { "nuten", "lsp" },
  filetypes = { "nu" },
  on_attach = require("lsp-format").on_attach,
})
vim.lsp.enable("nuten")
```

### Nix

::: code-group

```nix [nixvim]
{
  filetype.extension.nuon = "nu";
  lsp.servers.nuten = {
    enable = true;
    config = {
      cmd = [ "nuten" "lsp" ];
      filetypes = [ "nu" ];
    };
  };
  plugins.lsp-format = {
    enable = true;
    lspServersToEnable = [ "nuten" ];
  };
}
```

```nix [nvf]
{
  vim.filetype.extension.nuon = "nu";
  vim.lsp = {
    enable = true;
    formatOnSave = true;
    servers.nuten = {
      cmd = [ "nuten" "lsp" ];
      filetypes = [ "nu" ];
    };
  };
}
```

:::

## conform.nvim

Register Nuten as a formatter for [conform.nvim](https://github.com/stevearc/conform.nvim). This does not use the language server, and formats the buffer by passing it to `nuten format` via stdin.

```lua
require("conform").setup({
  formatters = {
    nuten = {
      command = "nuten",
      args = { "format", "--stdin-filepath", "$FILENAME", "-" },
    },
  },
  formatters_by_ft = { nu = { "nuten" } },
  format_on_save = { timeout_ms = 500 },
})
```

Since `--stdin-filepath` passes the buffer's path, Nuten looks for the config file based on that path.

### Nix

::: code-group

```nix [nixvim]
{
  filetype.extension.nuon = "nu";
  plugins.conform-nvim = {
    enable = true;
    settings = {
      formatters.nuten = {
        command = "nuten";
        args = [ "format" "--stdin-filepath" "$FILENAME" "-" ];
      };
      formatters_by_ft.nu = [ "nuten" ];
      format_on_save.timeout_ms = 500;
    };
  };
}
```

```nix [nvf]
{
  vim.filetype.extension.nuon = "nu";
  vim.formatter.conform-nvim = {
    enable = true;
    setupOpts = {
      formatters.nuten = {
        command = "nuten";
        args = [ "format" "--stdin-filepath" "$FILENAME" "-" ];
      };
      formatters_by_ft.nu = [ "nuten" ];
      format_on_save.timeout_ms = 500;
    };
  };
}
```

:::

## coc.nvim

Register `nuten lsp` as a language server for [coc.nvim](https://github.com/neoclide/coc.nvim) in `coc-settings.json` and format on save. Neovim 0.8 or later and Node.js are required.

```json [coc-settings.json]
{
  "languageserver": {
    "nuten": {
      "command": "nuten",
      "args": ["lsp"],
      "filetypes": ["nu"]
    }
  },
  "[nu]": {
    "coc.preferences.formatOnSave": true
  }
}
```

### Nix

With home-manager, write the contents of `coc-settings.json` in `programs.neovim.coc.settings`. Since home-manager has no filetype option, assign `.nuon` files the `nu` filetype in Lua.

```nix
{
  programs.neovim = {
    enable = true;
    coc = {
      enable = true;
      settings = {
        languageserver.nuten = {
          command = "nuten";
          args = [ "lsp" ];
          filetypes = [ "nu" ];
        };
        "[nu]"."coc.preferences.formatOnSave" = true;
      };
    };
  };
}
```
