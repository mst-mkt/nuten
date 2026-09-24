let list_over_lines = (
    [$alpha] ++ [
        $beta
        $gamma
    ]
    | length
)
let string_over_lines = (
    "a
b" ++ "the second part of the text" ++ "the third part of the text" ++ "the fourth part of the text"
)
let bytes_over_lines = (
    0x[01 02 03 04 05 06 07 08 09 0a] ++ 0x[0b 0c 0d 0e 0f 10 11 12 13 14] ++ 0x[
        15
    ] ++ 0x[16]
)
let tail_over_lines = (
    $alpha | $in + $beta + $gamma + $delta + $epsilon + $zeta + $eta + $theta + {
        a: 1
    }.a
)
