use std/assert
use std/testing *

source ../lib/prepare.nu

const GROUP = {
    name: a
    tag: 'a/v{version}'
    paths: [a]
    changelog: a/CHANGELOG.md
    versions: [
        { file: a/package.json, field: [version] }
        { file: a/jsr.json, field: [version] }
        { file: a/jsr.json, field: [imports, b], value: 'jsr:@scope/b@{version}' }
    ]
    locks: [a/Cargo.lock]
}

const RELEASED = "# Changelog\n\n## 0.1.0 - 2026-01-02\n\n🎉 Initial release\n"

def texts-of [
    version: string
    changelog: oneof<string, nothing>
]: nothing -> table<file: string, text: any> {
    [
        { file: a/package.json, text: $"{\n  \"version\": \"($version)\"\n}\n" }
        {
            file: a/jsr.json
            text: $"{\n  \"version\": \"($version)\",\n  \"imports\": {\n    \"b\": \"jsr:@scope/b@($version)\"\n  }\n}\n"
        }
        { file: a/Cargo.lock, text: $"[[package]]\nname = \"a\"\nversion = \"($version)\"\n" }
        { file: a/CHANGELOG.md, text: $changelog }
    ]
}

def written [plan: record, file: string]: nothing -> oneof<string, nothing> {
    $plan.writes | where file == $file | get 0?.text
}

@test
def "files lists places and changelog of group" [] {
    assert equal (files $GROUP) [a/package.json, a/jsr.json, a/Cargo.lock, a/CHANGELOG.md]
}

@test
def "prepare bumps version by part" [] {
    let actual = main $GROUP (texts-of 0.0.0 null) null [] minor 2026-01-02

    assert equal [$actual.version, $actual.tag] [0.1.0, a/v0.1.0]
}

@test
def "prepare starts changelog at first release" [] {
    let actual = main $GROUP (texts-of 0.0.0 null) null [] minor 2026-01-02

    assert equal (written $actual a/CHANGELOG.md) $RELEASED
}

@test
def "prepare writes each file of versions once" [] {
    let actual = main $GROUP (texts-of 0.0.0 null) null [] minor 2026-01-02

    assert equal ($actual.writes | get file | sort) [a/CHANGELOG.md, a/jsr.json, a/package.json]
}

@test
def "prepare bumps every version in file" [] {
    let actual = main $GROUP (texts-of 0.0.0 null) null [] minor 2026-01-02

    let expected = texts-of 0.1.0 null | where file == a/jsr.json | get 0.text
    assert equal (written $actual a/jsr.json) $expected
}

@test
def "prepare leaves locks to cargo" [] {
    let actual = main $GROUP (texts-of 0.0.0 null) null [] minor 2026-01-02

    assert equal $actual.locks [a/Cargo.lock]
    assert equal (written $actual a/Cargo.lock) null
}

@test
def "prepare lists changes since released version above earlier sections" [] {
    let subjects = ['fix(a): keep the comment', 'docs: describe a']

    let actual = main $GROUP (texts-of 0.1.0 $RELEASED) 0.1.0 $subjects patch 2026-02-03

    let expected = "# Changelog\n\n## 0.1.1 - 2026-02-03\n\n### Fixes\n\n- **a:** keep the comment\n\n## 0.1.0 - 2026-01-02\n\n🎉 Initial release\n"
    assert equal (written $actual a/CHANGELOG.md) $expected
}

@test
def "prepare says no notable changes after release without commits" [] {
    let actual = main $GROUP (texts-of 0.1.0 $RELEASED) 0.1.0 [] patch 2026-02-03

    assert equal (written $actual a/CHANGELOG.md | changelog notes 0.1.1) 'No notable changes'
}

@test
def "prepare rejects version that has no tag yet" [] {
    let message = try {
        main $GROUP (texts-of 0.1.0 null) null [] patch 2026-01-02
    } catch {|error|
        $error.msg
    }

    assert str contains $message 'a is at 0.1.0, which has no tag'
}

@test
def "prepare rejects group whose places disagree" [] {
    let disagreeing = texts-of 0.1.0 null | update 0.text "{\n  \"version\": \"0.0.9\"\n}\n"

    let message = try {
        main $GROUP $disagreeing 0.1.0 [] patch 2026-01-02
    } catch {|error|
        $error.msg
    }

    assert str contains $message 'the places of a disagree on its version'
}
