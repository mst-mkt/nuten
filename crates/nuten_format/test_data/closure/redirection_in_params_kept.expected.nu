ls | each {|x o> y| $x }
ls | each {|o> x| $x }
ls | each {|x e> | $x } {|x = e> | $x } {|x: int err> | $x }
