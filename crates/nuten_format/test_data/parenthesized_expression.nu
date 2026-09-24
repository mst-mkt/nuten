let alpha = 1; let beta = 2; let gamma = 3; let delta = 4; let epsilon = 5; let zeta = 6
let eta = 7; let theta = 8; let iota = 9; let kappa = 10; let lambda = 11; let mu = 12
let fits = ($alpha + $beta)
let sum = ($alpha + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + $iota + $kappa + $lambda + $mu)
let mixed = ($alpha * $beta + $gamma * $delta + $epsilon * $zeta + $eta * $theta + $iota * $kappa * $lambda * $mu)
let condition = (($alpha | into string) == "repositories" and ($beta | into string) == "url" and ($gamma | into string) == "https://example.com")
let negated = (not ($alpha == 1000000000000000000000000000000) and not ($beta == 20000000000000000000000000000000000000))
let before = ($alpha
    + $beta + $gamma)
let after = ($alpha +
    $beta + $gamma)
let commented = ($alpha + # c
    $beta + $gamma)
let last = ($alpha > 0 and ([$beta $gamma $delta $epsilon $zeta $eta $theta $iota $kappa $lambda $mu] | length) > 0)
let piped = (ls | where $it.name =~ "a_long_pattern_to_look_for" and $it.size > 100000000000000000000000000 and $it.type == file)
let declared = (let inner = $alpha + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + $iota + $kappa + $lambda + $mu)
let text = $"a($alpha + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + $iota + $kappa + $mu)b"
def outside [] { 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10 + 11 + 12 + 13 + 14 + 15 + 16 + 17 + 18 + 19 + 20 + 21 }
let call = (str replace --all "a_long_pattern_to_look_for" "a_long_replacement_to_put_there" "the input text")
let only_not = (not ($alpha == 1000000000000000000000000000000000000000000000000000000000000000000000000000000000))
let piped_math = (1 | $in + $alpha + $beta + $gamma + $delta + $epsilon + $zeta + $alpha + $beta + $gamma + $delta + $epsilon)
let either = ($alpha == 1 and $beta == 2 or $gamma == 3 and $delta == 4 or $epsilon == 5 and $zeta == 600000000000)
let exclusive = ($alpha == 1 and $beta == 2 xor $gamma == 3 and $delta == 4 xor $epsilon == 5 and $zeta == 6000000000000)
let assigned = ($env.TOTAL = $alpha + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + $iota + $kappa + $lambda)
let shared = ($alpha + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + $iota + $kappa + $lambda; $in)
let list_over_lines = ([$alpha] ++ [
    $beta
    $gamma
] | length)
let closure_over_lines = ($alpha == ([1 2] | each {|x|
    $x
}))
let text_over_lines = $"a($alpha
    + $beta + $gamma)b"
let emptied = ([$alpha $beta $gamma $delta $epsilon $zeta] ++ [$eta $theta $iota $kappa $lambda $mu] ++ [
] ++ [$alpha])
let string_over_lines = ("a
b" ++ "the second part of the text" ++ "the third part of the text" ++ "the fourth part of the text")
let word_as_term = ($alpha + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + $iota + $kappa + $lambda + $mu == and)
let bytes_over_lines = (0x[01 02 03 04 05 06 07 08 09 0a] ++ 0x[0b 0c 0d 0e 0f 10 11 12 13 14] ++ 0x[
    15
] ++ 0x[16])
let nested_over_lines = ($alpha + ($beta
    + 1) + $alpha + $beta + $alpha + $beta + $alpha + $beta + $alpha + $beta + $alpha + $beta)
let tail_over_lines = ($alpha | $in + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + {
    a: 1
}.a)
let not_alone = ($alpha | not ($alpha == 1000000000000000000000000000000000000000000000000000000000000000000000))
