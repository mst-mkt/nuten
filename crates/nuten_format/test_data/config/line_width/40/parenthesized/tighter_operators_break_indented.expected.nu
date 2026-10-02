let a = 1; let b = 2; let c = 3; let d = 4; let e = 5; let f = 6
let alpha = 1; let bravo = 2; let charlie = 3
print (
    $a * $b + $c * $d - $e > $f
    and $a < $b
)
print (
    $a * $b
            + $c * $d
            - $e
            + $f * $a * $b
        > $c
    and $a < $b
)
print (
    $alpha
                * $bravo
                * $charlie
                * $alpha
                * $bravo
            + $e
        > $f
    and $a < $b
)
print (
    $a == 1
    and (
        [
            $a
            $b
            $c
            $d
            $e
            $f
            $a
            $b
            $c
            $d
            $e
            $f
        ]
        | length
    )
        > 0
)
print (
    not $alpha and not $bravo
    or $charlie and $alpha
)
