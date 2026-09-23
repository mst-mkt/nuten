use std/testing *
@example  "a"  {  ls  }  --result  {a:  1}  # the description
@search-terms  a  b
@category  x;  def  --env  f  [a:int]  {ls}
@complete  external
extern  e  [--x]
@category  x
export  def  g  []  {}
@category  x;@deprecated
export  extern  h  []
@test  {a:  1}
def  t  []  {}
module m {
  @category  x
  export def i [] {
    @category  y
    def j [] {}
  }
}
@category x
def
(
let x = @category "y"
export extern h []
)
