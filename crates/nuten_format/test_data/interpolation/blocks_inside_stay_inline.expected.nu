let y = 1
print $'x([1, 2, 3] | each {|v| $v * 2 } | str join ",")'
print $"i(if $y == 1 { 'one' } else { 'other' })"
print $"t(try { 1 } catch { 2 })"
print $"dd(do { $y })"
let r = $"z(def g [] { 1 }; g)"
