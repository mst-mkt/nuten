let mixed = (
    $alpha * $beta
    + $gamma * $delta
    + $epsilon * $zeta
    + $eta * $theta
    + $iota * $kappa * $lambda * $mu
)
let condition = (
    ($alpha | into string) == "repositories"
    and ($beta | into string) == "url"
    and ($gamma | into string) == "https://example.com"
)
let last = (
    $alpha > 0
    and ([$beta $gamma $delta $epsilon $zeta $eta $theta $iota $kappa $lambda $mu] | length) > 0
)
let either = (
    $alpha == 1 and $beta == 2
    or $gamma == 3 and $delta == 4
    or $epsilon == 5 and $zeta == 600000000000
)
let exclusive = (
    $alpha == 1 and $beta == 2
    xor $gamma == 3 and $delta == 4
    xor $epsilon == 5 and $zeta == 6000000000000
)
