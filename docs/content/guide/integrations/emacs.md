---
title: Emacs
---

# Emacs

Set up Nuten with one of Emacs's built-in Eglot, the lsp-mode package, or the Apheleia package. All methods use the `nuten` on the PATH, so [install](/guide/installation) `nuten` first. To use a `nuten` that is not on the PATH, replace `"nuten"` in the examples with its path.

Emacs does not include a major mode for Nushell, so you also need a package such as [nushell-mode](https://github.com/mrkkrp/nushell-mode). The examples below assume nushell-mode. To use nushell-ts-mode, install the Nushell tree-sitter grammar as well, and replace `nushell-mode` with `nushell-ts-mode`.

nushell-mode turns on automatically only for `.nu` files. To format `.nuon` files too, assign them nushell-mode with the following setting.

```elisp
(add-to-list 'auto-mode-alist '("\\.nuon\\'" . nushell-mode))
```

## Eglot

Register `nuten lsp` as a language server and format on save. Eglot is built into Emacs 29 and later.

```elisp
(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs '(nushell-mode . ("nuten" "lsp"))))
(add-hook 'nushell-mode-hook
          (lambda ()
            (eglot-ensure)
            (add-hook 'before-save-hook #'eglot-format-buffer nil t)))
```

Eglot uses only one language server per buffer, so this replaces the Nushell language server that Eglot uses by default. To keep the Nushell language server for completion and other features, keep Eglot's default setup and use Apheleia only for formatting, or use lsp-mode.

## lsp-mode

Register `nuten lsp` as a language server for [lsp-mode](https://github.com/emacs-lsp/lsp-mode) and format on save.

```elisp
(with-eval-after-load 'lsp-mode
  (lsp-register-client
   (make-lsp-client
    :new-connection (lsp-stdio-connection '("nuten" "lsp"))
    :activation-fn (lsp-activate-on "nushell")
    :server-id 'nuten
    :add-on? t)))
(add-hook 'nushell-mode-hook
          (lambda ()
            (lsp-deferred)
            (add-hook 'before-save-hook #'lsp-format-buffer nil t)))
```

With `:add-on? t`, Nuten runs alongside the Nushell language server.

## Apheleia

Register Nuten as a formatter for [Apheleia](https://github.com/radian-software/apheleia). This does not use the language server, and formats the buffer by passing its contents to `nuten format` via stdin.

```elisp
(with-eval-after-load 'apheleia
  (setf (alist-get 'nuten apheleia-formatters)
        '("nuten" "format" "--stdin-filepath" filepath "-"))
  (setf (alist-get 'nushell-mode apheleia-mode-alist) 'nuten))
(add-hook 'nushell-mode-hook #'apheleia-mode)
```

Since `--stdin-filepath` also passes the buffer's path, Nuten looks for the config file based on that path.
