# https://github.com/nushell/nufmt/issues/199

let a = [
    { pi update } # races with npm update
    { yay-uu }
]
let b = [
    # { pi update }
    { yay-uu }
]
