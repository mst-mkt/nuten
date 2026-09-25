# https://github.com/nushell/nufmt/issues/185

export module allexported {
    export alias g = git
}
export module modexported {
    alias g = git
}
module aliasexported {
    export alias g = git
}
