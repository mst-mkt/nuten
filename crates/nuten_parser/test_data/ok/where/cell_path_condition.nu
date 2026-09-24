ls | where $it.a > 1
ls | where {a: 1}.a
ls | where (ls).a
