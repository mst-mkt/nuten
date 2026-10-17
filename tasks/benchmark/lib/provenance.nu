export def versions [repository: path, lock: path]: nothing -> record {
    { nuten: (nuten-version $repository), ...(locked-versions $lock) }
}

def nuten-version [repository: path]: nothing -> oneof<string, nothing> {
    if (which git | is-empty) {
        return null
    }
    let described = ^git -C $repository describe --always --dirty | complete

    if $described.exit_code == 0 {
        $described.stdout | str trim
    } else {
        null
    }
}

def locked-versions [lock: path]: nothing -> record {
    if not ($lock | path exists) {
        return { nufmt: null, nushell: null, corpus: null }
    }
    let nodes = open --raw $lock | from json | get nodes

    {
        nufmt: ($nodes | locked nufmt)
        nushell: ($nodes | locked nushell)
        corpus: ($nodes | corpus-versions)
    }
}

def corpus-versions []: record -> record {
    let nodes = $in

    $nodes
    | columns
    | where $it starts-with corpus-
    | each {|name| { name: ($name | str replace corpus- ''), locked: ($nodes | locked $name) } }
    | transpose --header-row --as-record
}

def locked [name: string]: record -> oneof<record, nothing> {
    let node = $in | get --optional $name
    if $node == null {
        return null
    }

    { rev: $node.locked.rev, date: ($node.locked.lastModified * 1_000_000_000 | into datetime) }
}

export def environment []: nothing -> record {
    let host = sys host
    let cpus = sys cpu

    {
        os: $host.long_os_version
        kernel: $host.kernel_version
        cpu: ($cpus | first | get brand)
        cpus: ($cpus | length)
    }
}
