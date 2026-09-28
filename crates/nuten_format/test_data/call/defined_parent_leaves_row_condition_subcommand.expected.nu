def take [n: int] {}
[1, 2] | take until {a:1}
module m {
    export def take [n: int] {}
}
do {
    use m take
    [1, 2] | take until {a:1}
}
