alias a = ls | [a,b]
alias b = ^ls | {a:1}
alias c = ^ls || [a,b]
alias d = ls o> [a,b]
alias e = ^ls e>| {a:1}
