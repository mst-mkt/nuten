def else [x] { $x }
else {a:  1}
def catch [x] { $x }
catch {a:  1}
def "if x" [y] { $y }
if x {  1 }
if $true {  1 } else {  2 }
def "try x" [y] { $y }
try x {  1 }
def "return x" [y: string] { $y }
return x [a,  b]
def any [x] { $x }
any {a:  1}
do { def all [x] { $x } }
all {a:  1}
def "while x" [y] { $y }
use m.nu *
while x {  1 }
