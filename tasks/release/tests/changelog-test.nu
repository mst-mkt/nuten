use std/assert
use std/testing *

source ../lib/changelog.nu

@test
def "entries keep only types of sections" [] {
    let subjects = [
        'feat: a'
        'refactor: b'
        'fix: c'
        'chore: d'
        'add: e'
        'perf: f'
        'merge branch main'
    ]

    assert equal ($subjects | entries | get summary) [a, c, e, f]
}

@test
def "entries split scope from summary" [] {
    let actual = ['feat(format): keep the blank line', 'fix: read a word']
        | entries
        | select scope summary

    assert equal $actual [{ scope: format, summary: 'keep the blank line' }, { scope: '', summary: 'read a word' }]
}

@test
def "entries read breaking mark" [] {
    let actual = ['feat(cli)!: drop the --check flag', 'feat: keep'] | entries | get breaking

    assert equal $actual [true, false]
}

@test
def "entries keep breaking change of any type" [] {
    assert equal (['refactor!: rename the config'] | entries | get type) [refactor]
}

@test
def "entries drop subject with empty summary" [] {
    assert equal (['feat: ', 'fix:'] | entries) []
}

@test
def "body groups entries by section in order of sections" [] {
    let entries = [
        { type: fix, scope: '', breaking: false, summary: b }
        { type: feat, scope: format, breaking: false, summary: a }
        { type: perf, scope: parser, breaking: false, summary: e }
        { type: add, scope: '', breaking: false, summary: d }
        { type: fix, scope: cli, breaking: false, summary: c }
    ]

    let actual = $entries | body

    let expected = "### Features\n\n- **format:** a\n\n### Additions\n\n- d\n\n### Fixes\n\n- b\n- **cli:** c\n\n### Performance\n\n- **parser:** e"
    assert equal $actual $expected
}

@test
def "body puts breaking changes first and only there" [] {
    let entries = [
        { type: feat, scope: '', breaking: false, summary: a }
        { type: feat, scope: cli, breaking: true, summary: b }
        { type: refactor, scope: '', breaking: true, summary: c }
    ]

    let actual = $entries | body

    assert equal $actual "### Breaking changes\n\n- **cli:** b\n- c\n\n### Features\n\n- a"
}

@test
def "body is no notable changes for no entries" [] {
    assert equal ([] | body) 'No notable changes'
}

@test
def "body is no notable changes for entries of other types" [] {
    let entries = [{ type: docs, scope: '', breaking: false, summary: a }]

    assert equal ($entries | body) 'No notable changes'
}

@test
def "prepend starts changelog when there is none" [] {
    let actual = null | prepend (section 0.1.0 2026-01-02 a)

    assert equal $actual "# Changelog\n\n## 0.1.0 - 2026-01-02\n\na\n"
}

@test
def "prepend starts changelog over blank text" [] {
    let actual = " \n\n" | prepend (section 0.1.0 2026-01-02 a)

    assert equal $actual "# Changelog\n\n## 0.1.0 - 2026-01-02\n\na\n"
}

@test
def "prepend puts section above newest one" [] {
    let text = "# Changelog\n\n## 0.2.0 - 2026-02-03\n\nb\n\n## 0.1.0 - 2026-01-02\n\na\n"

    let actual = $text | prepend (section 0.3.0 2026-03-04 c)

    assert equal $actual "# Changelog\n\n## 0.3.0 - 2026-03-04\n\nc\n\n## 0.2.0 - 2026-02-03\n\nb\n\n## 0.1.0 - 2026-01-02\n\na\n"
}

@test
def "prepend puts section after heading of changelog without sections" [] {
    let actual = ["# Changelog", "# Changelog\n", "# Changelog\n\n"]
        | each { prepend (section 0.1.0 2026-01-02 a) }
        | uniq

    assert equal $actual ["# Changelog\n\n## 0.1.0 - 2026-01-02\n\na\n"]
}

@test
def "notes take body of section of version" [] {
    let text = "# Changelog\n\n## 0.2.0 - 2026-02-03\n\n### Fixes\n\n- b\n\n## 0.1.0 - 2026-01-02\n\na\n"

    assert equal ($text | notes 0.2.0) "### Fixes\n\n- b"
}

@test
def "notes take last section that does not end with line break" [] {
    assert equal ("## 0.2.0 - 2026-02-03\n\nb\n\n## 0.1.0 - 2026-01-02\n\na" | notes 0.1.0) a
}

@test
def "notes take section whose heading has no date" [] {
    assert equal ("## 0.1.0\n\na\n" | notes 0.1.0) a
}

@test
def "notes read changelog with crlf" [] {
    assert equal ("## 0.1.0\r\n\r\na\r\nb\r\n" | notes 0.1.0) "a\nb"
}

@test
def "notes do not take section of version with same prefix" [] {
    let text = "## 0.1.10 - 2026-02-03\n\nb\n\n## 0.1.1 - 2026-01-02\n\na\n"

    assert equal ($text | notes 0.1.1) a
}

@test
def "notes fail for version without section" [] {
    let message = try { "## 0.1.0 - 2026-01-02\n\na\n" | notes 0.2.0 } catch {|error| $error.msg }

    assert str contains $message 'the changelog has no section for 0.2.0'
}
