let y = 1
print $"a($y + 1)b"
print $"a($"b($y + 1)c")d"
print $'x([1 2 3] | each {|v| $v * 2 } | str join ",")'
print $"r({ a: 1 b: 2 } | get a)"
print $"i(if $y == 1 { 'one' } else { 'other' })"
print $"m(match $y {1 => 'a' _ => 'b'})"
print $"t(try { 1 } catch { 2 })"
print $"(
    [1 2]
    | length
)"
print $"p()q"
print $"\(literal) ($y)"
print $"q('(' + ")")"
print $"n(1..3 | math sum)"
print $"c($y | into string | str length)"
print $"dd(do { $y })"
let r = $"z(def g [] { 1 }; g)"
print ($"($y)" + $'($y)')
[$"($y)" $"($y)"] | print
{ k: $"($y)" } | print
print $"(ls | where size > 1kb | length)"
print $"(((1)))"
print $"('a' ++ "b")"
print $"(
    # c
    1
)"
print $"(
    1
)"
print $"( "a
b"  |  str length )"
