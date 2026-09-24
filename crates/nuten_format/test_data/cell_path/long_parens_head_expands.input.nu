(ls | where size > 1kb | sort-by modified | reverse | first 10 | get name | str join ", " | str trim).0
print (ls | where size > 1kb | sort-by modified | reverse | first 10 | get name | str join ",").abcdefg
