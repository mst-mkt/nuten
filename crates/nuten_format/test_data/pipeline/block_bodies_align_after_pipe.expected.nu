ls
| each {|x|
      $x
      | each {|y|
            $y
        }
  }
let x = ls
    | each {|x|
          $x
      }
ls
| get {
      a: 1
  }
