if $x in $y..$z { }
match $x..$y { _ => $x..$y }
if true { } else 1..3
if true { } else 1..$n
match $x { _ => 1..3, _ => 1..$n }
