let closure_over_lines = (
    $alpha == (
        [1 2] | each {|x|
            $x
        }
    )
)
let nested_over_lines = (
    $alpha + (
        $beta
        + 1
    ) + $alpha + $beta + $alpha + $beta + $alpha + $beta + $alpha + $beta + $alpha + $beta
)
