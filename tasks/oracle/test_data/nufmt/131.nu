# https://github.com/nushell/nufmt/issues/131

def call [x] { $x }
def f [] {
   if "a" != "b" {
          return (call true)
        }
}
