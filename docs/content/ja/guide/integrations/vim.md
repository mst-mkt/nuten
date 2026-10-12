---
title: Vim
---

# Vim

プラグインの ALE, vim-lsp, yegappan/lsp, coc.nvim のいずれかで設定できます。どの方法も PATH 上の `nuten` を呼び出すので、先に `nuten` の [インストール](/ja/guide/installation) が必要です。PATH にない `nuten` を使う場合は、例の `nuten` をそのパスに置き換えてください。

Vim が filetype `nu` と判定するのは、9.0.2131 以降の `.nu` だけです。`.nuon` も整形したい場合や古い Vim を使う場合は、次の設定で filetype `nu` を割り当てます。

```vim
autocmd BufRead,BufNewFile *.nu,*.nuon setfiletype nu
```

## ALE

[ALE](https://github.com/dense-analysis/ale) の fixer として登録します。言語サーバーは使わず、バッファの内容を標準入力で `nuten format` に渡して整形します。

```vim
augroup nuten
  autocmd!
  autocmd FileType nu let b:ale_fixers = [{buffer -> {'command': 'nuten format --stdin-filepath %s -'}}]
  autocmd FileType nu let b:ale_fix_on_save = 1
augroup END
```

`--stdin-filepath` でバッファのパスも渡しているので、Nuten はそのパスを基準に設定ファイルを探します。

## vim-lsp

[vim-lsp](https://github.com/prabirshrestha/vim-lsp) に `nuten lsp` を言語サーバーとして登録し、保存時に整形します。

```vim
function! s:on_lsp_buffer_enabled() abort
  if &filetype ==# 'nu'
    autocmd! nuten BufWritePre <buffer> LspDocumentFormatSync
  endif
endfunction

if executable('nuten')
  augroup nuten
    autocmd!
    autocmd User lsp_setup call lsp#register_server({
    \   'name': 'nuten',
    \   'cmd': {server_info -> ['nuten', 'lsp']},
    \   'allowlist': ['nu'],
    \ })
    autocmd User lsp_buffer_enabled call s:on_lsp_buffer_enabled()
  augroup END
endif
```

## yegappan/lsp

[yegappan/lsp](https://github.com/yegappan/lsp) に `nuten lsp` を言語サーバーとして登録し、保存時に整形します。Vim 9.0 以降が必要です。

```vim
function! s:on_lsp_attached() abort
  if &filetype ==# 'nu'
    autocmd! nuten BufWritePre <buffer> LspFormat
  endif
endfunction

augroup nuten
  autocmd!
  autocmd User LspSetup call LspAddServer([#{
  \   name: 'nuten',
  \   filetype: ['nu'],
  \   path: 'nuten',
  \   args: ['lsp'],
  \ }])
  autocmd User LspAttached call s:on_lsp_attached()
augroup END
```

## coc.nvim

[coc.nvim](https://github.com/neoclide/coc.nvim) の `coc-settings.json` に `nuten lsp` を言語サーバーとして登録し、保存時に整形します。Vim 9.0.0438 以降と Node.js が必要です。

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
