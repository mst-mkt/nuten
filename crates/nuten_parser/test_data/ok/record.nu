echo {a: 1, b: 2} {a:1,b:2}
{a: [1, 2], b: {c: 3}} | to json
echo {a: b:c} {a: b : c} {"a:b": 1} {a: :b}
echo {...$x, a: 1} {a: 1 ...(f)} {...{a: 1}}
echo {a: 1 b} {a: } {a 1 2} {a: 1 | b} {a: 1; b: 2}
echo {|x| $x} {} { } {a b} {a,b: 1} {...x} {,a: 1} {,: 1} {...$x,a: 1} {...$x ,: 1} {,...$x,a: 1}
^echo {a: 1} [{a: 1}]
echo {a: b:#c d} {x: y:e>|z} {a: e>|b}
echo {a: 1}.a {a: 1}b
echo {
    a: 1 # c

    b:
        2
}
ls o> {a: 1} {b: 2}
^echo o> {a: 1} {b: 2}
echo {|: 1} {||: a} {&&: 1} {2>&1: x} {o>|: 1} {out>|: 1} {2>: 1}
echo {...[1]} {....$x} {... $x}
echo {a: x:o>|} {a: o>|b} {x: y:o+e>|z} {a: o+e>|b} {a: b|c} {a: ||b}
echo {[a  b]: 1}
echo {a # c
: 1} {a: # c
 1}
echo {a,#{
}: 1}
echo {a: x:out>|} {a: out>|b} {a:
 b:c} {a:,b:c} {...$x k [a b]}
echo {a: $env.x#:[b c: d]
}
echo {a: b:#c
}
echo {a: k#,"x:}": 2
}
