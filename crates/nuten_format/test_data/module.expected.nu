module a
module b.nu
module c --help {x}
module d {x} --help
module -h (ls|x) {x}
module - {x} {y}
module e f
module -a {x}
module (ls|x)
module
module f {
    const x = 1
}
module g {
    export def h [] {
        1
    }; export def i [] {
        2
    }
}
module "j k" {
    export def l [] {}
}
module m { }
module n {}
module o {
    # c
    const x = 1

    const y = 2 # d
}
do {
    module p {
        const x = 1
    }
}
(
    module q
    {
        const x = 1
    }
)
(
    module r # c
    {
        const x = 1
    }
)
module s { ls && y }
module (ls|x) {
    const x = 1
}
module {a:1} {
    const x = 1
}
module t {|x| def u [] {}}
module v {
| def w [] {}
}
module z {|| def u [] {}}
export module n {
    export const x = 1
}
export module o.nu
export module p {}
export module --help q
(
    export # c
    module m {}
)
