for x in [1 2] { print $x }
for $x in 1..3 { $x }
for x in (ls) {
  $x
}
while $a < 3 { $a += 1 }
loop { break }
try { ls } catch { print $in }
try { ls } catch {|e| print $e }
let x = try { 1 } catch { 2 }
ls | for x in $in { $x } | first
for x [1] { ls }
for x in [1]
for x in [1] {a: 1}
while true
loop extra { break }
try {|e| ls }
try { ls } catch
try { ls } catch {a: 1}
try { ls } catch { ls } extra
try { ls }
catch { ls }
for x in [1] { ls } o> f.txt
for x in [1] { ls } # c
for x in [1] { ls } { ls }
for x: int in [1] { print $x }
for {a} in [1] { ls }
while {|| true} { break }
try { ls } catch { ls } catch { ls }
try { ls } catch { ls } o> f.txt
loop {}
for
try
while
loop
loop {...$x, a: 1}
