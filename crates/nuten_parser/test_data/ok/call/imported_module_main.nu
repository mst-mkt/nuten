module n {
  export def main [x] { $x }
  export def "two words" [x] { $x }
}
use n
n {a:  1}
n two words {a:  1}
