use std/assert
use std/testing *

source ../lib/places.nu

const GROUP = {
    name: a
    versions: [
        { file: package.json, field: [version] }
        { file: jsr.json, field: [imports, b], value: 'jsr:@scope/b@{version}' }
    ]
    locks: [Cargo.lock]
}

def texts-of [
    package: string
    import: string
    locked: string
]: nothing -> table<file: string, text: string> {
    [
        { file: package.json, text: $'{ "version": "($package)" }' }
        { file: jsr.json, text: $'{ "imports": { "b": "jsr:@scope/b@($import)" } }' }
        { file: Cargo.lock, text: $"[[package]]\nname = \"a\"\nversion = \"($locked)\"\n" }
    ]
}

def message [action: closure]: nothing -> string {
    try {
        do $action; ''
    } catch {|error|
        $error.msg
    }
}

@test
def "files lists each file of group once" [] {
    let group = $GROUP | update versions { append { file: jsr.json, field: [version] } }

    assert equal (files $group) [package.json, jsr.json, Cargo.lock]
}

@test
def "text is nothing for file that was not read" [] {
    assert equal (text (texts-of 0.1.0 0.1.0 0.1.0) b.json) null
}

@test
def "existing-text fails for file that does not exist" [] {
    let texts = [{ file: a.json, text: null }]

    assert str contains (message { existing-text $texts a.json }) 'a.json does not exist'
}

@test
def "versions list version of each location and lock" [] {
    let actual = versions $GROUP (texts-of 0.1.0 0.1.0 0.1.0)

    assert equal $actual [
        { file: package.json, field: version, version: 0.1.0 }
        { file: jsr.json, field: imports.b, version: 0.1.0 }
        { file: Cargo.lock, field: package, version: 0.1.0 }
    ]
}

@test
def "agreed is version all places hold" [] {
    assert equal (agreed $GROUP (texts-of 0.1.0 0.1.0 0.1.0)) 0.1.0
}

@test
def "agreed fails when one place holds another version" [] {
    let actual = message { agreed $GROUP (texts-of 0.1.0 0.1.0 0.0.9) }

    let expected = 'the places of a disagree on its version: package.json (version) 0.1.0, jsr.json (imports.b) 0.1.0, Cargo.lock (package) 0.0.9'
    assert str contains $actual $expected
}

@test
def "version-at takes version from field of json" [] {
    assert equal (
        version-at { file: a.json, field: [version] } '{ "name": "a", "version": "1.2.3" }'
    ) 1.2.3
}

@test
def "version-at takes version out of value template" [] {
    let location = { file: a.json, field: [imports, b], value: 'jsr:@scope/b@{version}' }

    assert equal (version-at $location '{ "imports": { "b": "jsr:@scope/b@1.2.3" } }') 1.2.3
}

@test
def "version-at fails for value that is not version" [] {
    let actual = message { version-at { file: a.toml, field: [version] } "version = \"workspace\"\n" }

    assert str contains $actual 'a.toml holds workspace at version'
}

@test
def "version-at fails for file of unknown format" [] {
    let actual = message { version-at { file: a.yaml, field: [version] } "version: 1.2.3\n" }

    assert str contains $actual 'a.yaml is a .yaml file'
}

@test
def "rewrite changes only line of field" [] {
    let text = "[workspace.package]\nversion = \"0.1.0\"\n\n[workspace.dependencies]\nb = { version = \"0.1.0\" }\nc = \"0.1.0\"\n"

    let actual = rewrite { file: Cargo.toml, field: [workspace, package, version] } $text 0.1.0 0.2.0

    assert equal $actual ($text | str replace "version = \"0.1.0\"\n\n" "version = \"0.2.0\"\n\n")
}

@test
def "rewrite keeps indentation and comma of json" [] {
    let text = "{\n  \"name\": \"a\",\n  \"version\": \"0.1.0\",\n  \"private\": true\n}\n"

    let actual = rewrite { file: a.json, field: [version] } $text 0.1.0 0.2.0

    assert equal $actual "{\n  \"name\": \"a\",\n  \"version\": \"0.2.0\",\n  \"private\": true\n}\n"
}

@test
def "rewrite fills value template" [] {
    let location = { file: a.json, field: [imports, b], value: 'jsr:@scope/b@{version}' }
    let text = "{\n  \"imports\": {\n    \"b\": \"jsr:@scope/b@0.1.0\"\n  }\n}\n"

    let actual = rewrite $location $text 0.1.0 0.2.0

    assert equal (version-at $location $actual) 0.2.0
}

@test
def "rewrite keeps dollar of value" [] {
    let location = { file: a.json, field: [b], value: '$1@{version}' }

    let actual = rewrite $location "{\n  \"b\": \"$1@0.1.0\"\n}\n" 0.1.0 0.2.0

    assert equal $actual "{\n  \"b\": \"$1@0.2.0\"\n}\n"
}

@test
def "rewrite rejects text where two lines start with version" [] {
    let text = "[package]\nversion = \"0.1.0\"\n\n[other]\nversion = \"0.1.0\"\n"

    let actual = message { rewrite { file: a.toml, field: [package, version] } $text 0.1.0 0.2.0 }

    assert str contains $actual 'a.toml has 2 lines that start with version = "0.1.0", not 1'
}

@test
def "rewrite rejects text where no line holds current version" [] {
    let text = "[package]\nversion = \"0.3.0\"\n"

    let actual = message { rewrite { file: a.toml, field: [package, version] } $text 0.1.0 0.2.0 }

    assert str contains $actual 'a.toml has 0 lines'
}

@test
def "rewrite rejects text where rewritten line is not field" [] {
    let text = "[a]\nversion = \"0.1.0\"\n\n[b]\nversion = \"0.0.9\"\n"

    let actual = message { rewrite { file: a.toml, field: [b, version] } $text 0.1.0 0.2.0 }

    assert str contains $actual 'a.toml holds 0.0.9 at b.version once 0.2.0 is written'
}

@test
def "locked lists versions of packages without source" [] {
    let text = [
        '[[package]]'
        'name = "a"'
        'version = "0.1.0"'
        ''
        '[[package]]'
        'name = "b"'
        'version = "0.1.0"'
        ''
        '[[package]]'
        'name = "c"'
        'version = "9.9.9"'
        'source = "registry+https://github.com/rust-lang/crates.io-index"'
        ''
    ]

    assert equal (locked ($text | str join "\n")) [0.1.0]
}
