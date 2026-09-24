def "while x" [y] { $y }
use m.nu *
while x {  1 }
def imported [x] { $x }
use m.nu *
imported {a:  1}
def longer [...x] { $x }
longer word {a:  1}
