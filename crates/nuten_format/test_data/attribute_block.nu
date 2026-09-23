@example  "a"  {  ls  }  --result  {a:  1}  # the description
@search-terms  a  b
@category  x;  def  --env  f  [a:int]  {ls}
@complete  external
extern  e  [--x]
@category  x
export  def  g  []  {}
@category  x;@deprecated
export  extern  h  []
module m {
  @category  x
  export def i [] {
    @category  y
    def j [] {}
  }
}
@category  x  # c
@search-terms  y
def u [] {}
(@search-terms a
b; def v [] {})
(
let x = @category "y"
export extern w []
)
@category x
def x [] {}


ls
