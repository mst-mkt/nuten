---
title: Emacs
---

# Emacs

Emacs 組み込みの Eglot か、パッケージの lsp-mode, Apheleia のいずれかで設定できます。どの方法も PATH 上の `nuten` を呼び出すので、先に `nuten` の [インストール](/ja/guide/installation) が必要です。PATH にない `nuten` を使う場合は、例の `"nuten"` をそのパスに置き換えてください。

Nushell 用のメジャーモードは Emacs に含まれていないので、[nushell-mode](https://github.com/mrkkrp/nushell-mode) などのパッケージが別に必要です。以下の例は nushell-mode を前提にしています。nushell-ts-mode を使う場合は、Nushell の tree-sitter 文法も入れたうえで、`nushell-mode` を `nushell-ts-mode` に読み替えてください。

nushell-mode が自動で有効になるのは `.nu` だけです。`.nuon` も整形したい場合は、次の設定で nushell-mode を割り当てます。

```elisp
(add-to-list 'auto-mode-alist '("\\.nuon\\'" . nushell-mode))
```

## Eglot

`nuten lsp` を言語サーバーとして登録し、保存時に整形します。Eglot は Emacs 29 から組み込まれています。

```elisp
(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs '(nushell-mode . ("nuten" "lsp"))))
(add-hook 'nushell-mode-hook
          (lambda ()
            (eglot-ensure)
            (add-hook 'before-save-hook #'eglot-format-buffer nil t)))
```

Eglot はバッファごとに言語サーバーを 1 つしか使えないため、この設定では Eglot が既定で使う Nushell の言語サーバーが Nuten に置き換わります。補完などを Nushell の言語サーバーに任せたい場合は、Eglot は既定のままにして整形だけを Apheleia で行うか、lsp-mode を使ってください。

## lsp-mode

[lsp-mode](https://github.com/emacs-lsp/lsp-mode) に `nuten lsp` を言語サーバーとして登録し、保存時に整形します。

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

`:add-on? t` を指定しているので、Nuten は Nushell の言語サーバーと同時に動きます。

## Apheleia

[Apheleia](https://github.com/radian-software/apheleia) のフォーマッタとして登録します。言語サーバーは使わず、バッファの内容を標準入力で `nuten format` に渡して整形します。

```elisp
(with-eval-after-load 'apheleia
  (setf (alist-get 'nuten apheleia-formatters)
        '("nuten" "format" "--stdin-filepath" filepath "-"))
  (setf (alist-get 'nushell-mode apheleia-mode-alist) 'nuten))
(add-hook 'nushell-mode-hook #'apheleia-mode)
```

`--stdin-filepath` でバッファのパスも渡しているので、Nuten はそのパスを基準に設定ファイルを探します。
