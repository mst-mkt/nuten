# https://github.com/nushell/nufmt/issues/189

def foo [--editor: string] {
    match $editor {
        _ => {error make {msg: "unsupported editor", label: {text: "given here", span: (metadata $editor).span}}}
    }
}
