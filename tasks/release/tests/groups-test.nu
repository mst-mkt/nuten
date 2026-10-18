use std/assert
use std/testing *

use ../lib/tag.nu

const GROUPS = path self ../groups.nuon

@test
def "groups have names of their own" [] {
    let names = open $GROUPS | get name

    assert equal ($names | uniq) $names
}

@test
def "tag of each group holds placeholder once" [] {
    let actual = open $GROUPS | where { ($in.tag | split row '{version}' | length) != 2 } | get name

    assert equal $actual []
}

@test
def "tag of each group reads as that group alone" [] {
    let groups = open $GROUPS

    let actual = $groups
        | each {|group| { group: $group.name, read: (tag identify $groups (tag name $group 1.2.3)) } }
        | where { $in.read != { group: $in.group, version: 1.2.3 } }

    assert equal $actual []
}

@test
def "versions of groups are in json or toml" [] {
    let actual = open $GROUPS
        | get versions
        | flatten
        | where { ($in.file | path parse | get extension) not-in [json, toml] }

    assert equal $actual []
}
