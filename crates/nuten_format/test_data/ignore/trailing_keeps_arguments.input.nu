(foo
  --a  1  --b  2 # nuten-ignore
  --c  3
)

(ls | where
  $it.size  >  1 # nuten-ignore
)

alias x = foo |
  bar   baz # nuten-ignore

(alias w =
  foo   --a    1 # nuten-ignore
)
