def "if x" [y] { $y }
if x {  1 }
if $true {  1 } else {  2 }
def "try x" [y] { $y }
try x {  1 }
def "return x" [y: string] { $y }
return x [a,  b]
