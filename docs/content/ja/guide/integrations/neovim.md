---
title: Neovim
---

# Neovim

Neovim 組み込みの vim.lsp か、プラグインの conform.nvim, coc.nvim のいずれかで設定できます。どの方法も PATH 上の `nuten` を呼び出すので、先に `nuten` の [インストール](/ja/guide/installation) が必要です。PATH にない `nuten` を使う場合は、`cmd` や `command` にそのパスを書いてください。

Neovim が filetype `nu` と判定するのは `.nu` だけです。`.nuon` も整形したい場合は、次の設定で filetype `nu` を割り当てます。

```lua
vim.filetype.add({ extension = { nuon = "nu" } })
```

## vim.lsp

`nuten lsp` を言語サーバーとして登録し、保存時に整形します。Neovim 0.11 以降が必要です。

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

[lsp-format.nvim](https://github.com/lukas-reineke/lsp-format.nvim) を使っている場合は、保存時の整形を lsp-format.nvim の `on_attach` に任せられます。

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

[conform.nvim](https://github.com/stevearc/conform.nvim) のフォーマッタとして登録します。言語サーバーは使わず、バッファの内容を標準入力で `nuten format` に渡して整形します。

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

`--stdin-filepath` でバッファのパスも渡しているので、Nuten はそのパスを基準に設定ファイルを探します。

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

[coc.nvim](https://github.com/neoclide/coc.nvim) の `coc-settings.json` に `nuten lsp` を言語サーバーとして登録し、保存時に整形します。Neovim 0.8 以降と Node.js が必要です。

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

home-manager では、`programs.neovim.coc.settings` に `coc-settings.json` の内容を書きます。home-manager には filetype を割り当てるオプションがないので、`.nuon` の filetype は Lua で割り当ててください。

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
