let alpha = 1; let beta = 2; let gamma = 3; let delta = 4; let epsilon = 5; let zeta = 6
let eta = 7; let theta = 8; let iota = 9; let kappa = 10; let lambda = 11; let mu = 12
let declared = (let inner = $alpha + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + $iota + $kappa + $lambda + $mu)
let call = (str replace --all "a_long_pattern_to_look_for" "a_long_replacement_to_put_there" "the input text")
let assigned = ($env.TOTAL = $alpha + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + $iota + $kappa + $lambda)
let shared = ($alpha + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + $iota + $kappa + $lambda; $in)
