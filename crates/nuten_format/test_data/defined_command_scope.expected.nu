before { a: 1 }
def before [x] {
    $x
}
def with-closure [c: closure] {
    do $c
}
with-closure {|x| $x }
with-closure { print hi }
do {
    def inner [x] {
        $x
    }
}
inner {a:  1}
(
    def paren [x] {
        $x
    }; paren { a: 1 }
)
do {|y|
    late { a: 1 }; def late [x] {
        $x
    }
}
match 1 {
    1 => {
        def in-arm [x] {
            $x
        }; in-arm { a: 1 }
    }
    _ => {}
}
module mm {
    def m4 [x] {
        $x
    }; export def m5 [] {
        m4 { a: 1 }
    }
}
def enclosing [] {
    nested { a: 1 }; def nested [x] {
        $x
    }
}
def "two words" [x] {
    $x
}
two words { a: 1 }
two {a:  1}
def recursive [x] {
    recursive { a: 1 }
}
extern outer [x]
outer { a: 1 }
def imported-inside [x] {
    $x
}
do { use m.nu *; imported-inside {a:  1} }
def imported [x] {
    $x
}
use m.nu *
imported {a:  1}
def longer [...x] {
    $x
}
longer word {a:  1}
def hidden [x] {
    $x
}
hide hidden
hidden {a:  1}
