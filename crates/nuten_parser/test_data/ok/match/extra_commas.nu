match $x { 1 => a,, _ => b }
match $x { , 1 => a }
match $x { ,, }
match $x { 1, => a }
