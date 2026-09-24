module m { export def f [x] { $x }; export def g [x] { $x }; def h [x] { $x } }
use m f
f {a:  1}
g {a:  1}
use m *
g {a:  1}
h {a:  1}
do { use m [f, g]; g {a:  1} }
module n {
  export def main [x] { $x }
  export def "two words" [x] { $x }
}
use n
n {a:  1}
n two words {a:  1}
use other.nu *
f {a:  1}
