def prefix [...x] {}
prefix longer [1, 2]
module later {
    export def "prefix longer" [] {}
}
use later *
