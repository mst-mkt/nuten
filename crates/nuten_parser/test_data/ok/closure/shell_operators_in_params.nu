echo {|x||} {|a ||} {|x && y| 1} {|; x| 1} {|x o>| y| 1} {|2>| x} {|a || b| 1} {|x 2>&1 y| 1} {|x out>| y| 1}
do {|o> x| $x} {|x o> y| $x} {|x e> | $x}
