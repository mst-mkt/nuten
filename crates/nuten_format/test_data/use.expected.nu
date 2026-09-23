use std/assert
use std [assert log]
use m.nu [
    a
    b
]
use m.nu ...[a b]
use m.nu "path add"
use -h [a,b] [c d]
use -- -x [a b]
use --help m.nu { a: 1 }
use m.nu [[a]; [b]]
use (ls | get 0) [a b]
use
use m.nu --help [a b]
use -- -- [a b]
(
    use m.nu
    [a b]
)
use m.nu [ # c
    a
]
export use std [assert log]
export use -h [a,b] [c d]
export use
(
    export
    use m.nu [a b]
)
