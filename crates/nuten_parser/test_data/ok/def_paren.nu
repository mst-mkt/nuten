def f (x: int) { $x }
def g () { ls }
def h (): nothing -> string { "a" }
def i () : int -> int { 1 }
export def j (x) { $x }
extern k (--flag: string)
def l (x = (1 + 2)) { ls }
def m (--a(-b) c?: string = "d") { ls }
def n (
  # leading
  x # about x
) {
  ls
}
extern o (x) { ls }
def "p(q)" () { ls }
def r (x)(y) { ls }
def s (x)y { ls }
def t ():: int -> int { 1 }
def u ()] { ls }
def v (a,#c (
)
) { ls }
