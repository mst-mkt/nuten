---
title: Vim
---

# Vim

Set up Nuten with one of the ALE, vim-lsp, yegappan/lsp, or coc.nvim plugins. All methods use the `nuten` on the PATH, so [install](/guide/installation) `nuten` first. To use a `nuten` that is not on the PATH, replace `nuten` in the examples with its path.

Vim 9.0.2131 and later detect `.nu` as the `nu` filetype, but not `.nuon`. To format `.nuon` files too, or to use an older Vim, assign both `.nu` and `.nuon` files the `nu` filetype.

```vim
autocmd BufRead,BufNewFile *.nu,*.nuon setfiletype nu
```

## ALE

Register Nuten as a fixer for [ALE](https://github.com/dense-analysis/ale). This does not use the language server, and formats the buffer by passing it to `nuten format` via stdin.

```vim
augroup nuten
  autocmd!
  autocmd FileType nu let b:ale_fixers = [{buffer -> {'command': 'nuten format --stdin-filepath %s -'}}]
  autocmd FileType nu let b:ale_fix_on_save = 1
augroup END
```

Since `--stdin-filepath` passes the buffer's path, Nuten looks for the config file based on that path.

## vim-lsp

Register `nuten lsp` as a language server for [vim-lsp](https://github.com/prabirshrestha/vim-lsp) and format on save.

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

Register `nuten lsp` as a language server for [yegappan/lsp](https://github.com/yegappan/lsp) and format on save. Vim 9.0 or later is required.

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

Register `nuten lsp` as a language server for [coc.nvim](https://github.com/neoclide/coc.nvim) in `coc-settings.json` and format on save. Vim 9.0.0438 or later and Node.js are required.

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
