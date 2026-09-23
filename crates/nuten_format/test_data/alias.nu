alias ll = ls -l
alias   la   =   ls   -a
alias "cargo b"	= cargo build
alias a = ls | [a,b]
alias b = ^ls | {a:1}
alias c = ^ls || [a,b]
alias d = ls o> [a,b]
alias e = ^ls e>| {a:1}
alias f = ls |
    sort
alias g = ls
    | sort
alias h = ls | # c
    sort
alias i = ls
# c
| sort
alias j = if true { ls } else { ps }
alias k = foo = [a,b]
(alias l =
    ls -l)
alias m = ls |


| sort
alias n = ls |

print 1
do {
    alias o = ls |
        sort
}
let p = alias q = ls |
    sort
alias r = if true { ls } | sort
alias s = ls; print 1
ls; | alias t = ls | [a,b]
alias u = | {x:1}
alias v = o> f {x:1}
(alias w = ls
-l)
alias x = |
{x:1,y:2}
alias y =
| {x:1,y:2}
alias z = # c
| {x:1}
alias aa = || y
alias ab = = y
alias ac =
ls
alias ad =
# c
| {x:1}
export  alias  ll  =  ls  -l  |  sort
export alias aaaaaaaaaaaaaaa = ls --long --all --du --directory --mime-type --threads | sort-by size | reverse
export alias x = if true {1} else {2}
export alias y = ls
| sort
