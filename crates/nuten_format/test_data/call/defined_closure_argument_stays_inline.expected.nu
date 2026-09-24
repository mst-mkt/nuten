def with-closure [c: closure] {
    do $c
}
with-closure {|x| $x }
with-closure { print hi }
