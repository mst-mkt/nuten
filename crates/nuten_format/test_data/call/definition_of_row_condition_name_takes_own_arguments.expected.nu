module m {
    export def take [n: int] {}
}
use m take
do {
    def "take until" [x: record] {}
    [1, 2] | take until { a: 1 }
}
