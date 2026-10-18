use std/assert
use std/testing *

source ../lib/tag.nu

const GROUPS = [
    { name: a, tag: 'v{version}', versions: [{ file: a.json, field: [version] }], locks: [] }
    { name: b, tag: 'b/v{version}', versions: [{ file: b.json, field: [version] }], locks: [] }
    { name: c, tag: 'c/v{version}', versions: [{ file: c.json, field: [version] }], locks: [] }
]

def texts-of [version: string]: nothing -> table<file: string, text: string> {
    [{ file: a.json, text: $'{ "version": "($version)" }' }]
}

@test
def "name fills tag of group with version" [] {
    let actual = $GROUPS | each { name $in 0.1.0 }

    assert equal $actual [v0.1.0, b/v0.1.0, c/v0.1.0]
}

@test
def "identify finds group and version of tag" [] {
    let actual = [v0.1.0, b/v1.2.3, c/v0.0.1] | each { identify $GROUPS $in }

    assert equal $actual [
        { group: a, version: 0.1.0 }
        { group: b, version: 1.2.3 }
        { group: c, version: 0.0.1 }
    ]
}

@test
def "identify finds nothing for tag of no group" [] {
    let actual = [b/1.0.0, v1.0, d/v1.0.0, v0.1.0-rc.1, release] | where {
        (identify $GROUPS $in) != null
    }

    assert equal $actual []
}

@test
def "released is newest tagged version of group" [] {
    let tags = [v0.2.0, v0.10.0, v0.9.0, b/v1.0.0]

    let actual = $GROUPS | each { released $in $tags | default none }

    assert equal $actual [0.10.0, 1.0.0, none]
}

@test
def "due holds only for version newer than released one" [] {
    assert (due 0.2.0 0.1.0)
    assert not (due 0.1.0 0.1.0)
    assert not (due 0.0.9 0.1.0)
}

@test
def "due compares with unreleased version before first release" [] {
    assert not (due 0.0.0 null)
    assert (due 0.1.0 null)
}

@test
def "due-tag names tag for version newer than newest tag" [] {
    assert equal (due-tag ($GROUPS | first) (texts-of 0.2.0) [v0.1.0, b/v1.0.0]) v0.2.0
}

@test
def "due-tag is nothing for version already tagged" [] {
    assert equal (due-tag ($GROUPS | first) (texts-of 0.2.0) [v0.2.0]) null
}

@test
def "due-tag is nothing for version older than newest tag" [] {
    assert equal (due-tag ($GROUPS | first) (texts-of 0.2.0) [v0.3.0]) null
}

@test
def "due-tag fails when places disagree" [] {
    let group = $GROUPS | first | update versions { append { file: b.json, field: [version] } }
    let texts = [...(texts-of 0.2.0), { file: b.json, text: '{ "version": "0.1.0" }' }]

    let message = try { due-tag $group $texts [] } catch {|error| $error.msg }

    assert str contains $message 'the places of a disagree on its version'
}
