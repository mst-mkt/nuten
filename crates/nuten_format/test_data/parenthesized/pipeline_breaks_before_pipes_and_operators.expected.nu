let piped = (
    ls
    | where $it.name =~ "a_long_pattern_to_look_for"
      and $it.size > 100000000000000000000000000
      and $it.type == file
)
let piped_math = (
    1
    | $in
      + $alpha
      + $beta
      + $gamma
      + $delta
      + $epsilon
      + $zeta
      + $alpha
      + $beta
      + $gamma
      + $delta
      + $epsilon
)
