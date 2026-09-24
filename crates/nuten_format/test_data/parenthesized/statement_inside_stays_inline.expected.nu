let declared = (
    let inner = $alpha + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + $iota + $kappa + $lambda + $mu
)
let call = (
    str replace --all "a_long_pattern_to_look_for" "a_long_replacement_to_put_there" "the input text"
)
let assigned = (
    $env.TOTAL = $alpha + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + $iota + $kappa + $lambda
)
let shared = (
    $alpha + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + $iota + $kappa + $lambda; $in
)
