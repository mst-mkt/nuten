let alpha = 1; let beta = 2; let gamma = 3; let delta = 4; let epsilon = 5; let zeta = 6
let eta = 7; let theta = 8; let iota = 9; let kappa = 10; let lambda = 11; let mu = 12
let piped = (ls | where $it.name =~ "a_long_pattern_to_look_for" and $it.size > 100000000000000000000000000 and $it.type == file)
let piped_math = (1 | $in + $alpha + $beta + $gamma + $delta + $epsilon + $zeta + $alpha + $beta + $gamma + $delta + $epsilon)
