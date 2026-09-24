# nuten-ignore
let table = [
  [name   size];
  [a      1   ]
  [bb     22  ]
]

def main [] {
    # nuten-ignore
    if  $a  {  1  }  else  {  2  }
    if $a {
        1
    } else {
        2
    }
}

if $a {
    if $b {
        # nuten-ignore
        let text = "a
  b"  |  str trim
    }
}
