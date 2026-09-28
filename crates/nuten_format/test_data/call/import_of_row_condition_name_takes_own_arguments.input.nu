module m { export def any [x: record] {}; export def "take until" [x: record] {} }
module n { export def take [n: int] {} }
use m any
any {a:1}
use m "take until"
[1, 2] | take until {a:1}
do {
    def take [n: int] {}
    [1, 2] | take until {a:1}
}
do {
    use n take
    [1, 2] | take until {a:1}
}
