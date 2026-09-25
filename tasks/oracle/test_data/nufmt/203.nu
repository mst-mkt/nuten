# https://github.com/nushell/nufmt/pull/203

def foo [] { "z" }
def f [x y] {
    match $x {
      a => 1
      # a comment between arms
      b if ($y | foo) == 'z' => 2
    }
}
$env.config = {
  trim: {
    # wrapping or truncating
    methodology: wrapping
  }
}
let s = r#'
a # b
'#
# kept
let t = 'c'
