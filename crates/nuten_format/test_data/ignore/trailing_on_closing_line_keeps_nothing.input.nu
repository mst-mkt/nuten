let list = [
  1  2
]  # nuten-ignore

let names = (ls
  | where  size > 1  | get  name) # nuten-ignore

(
  ls
).name.size   # nuten-ignore

$"(
1
)(2)(3)" # nuten-ignore

let after_bracket = [
  1
  2   3] # nuten-ignore
