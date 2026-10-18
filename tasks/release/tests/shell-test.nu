use std/assert
use std/testing *

use support.nu [commit, git-in, put, repository, with-git]
use ../lib/places.nu
source ../lib/shell.nu

@before-each
def setup []: nothing -> record {
    { dir: (repository) }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
}

@test
def "read-files gives nothing for missing file" [] {
    let context = $in

    assert equal (read-files $context.dir [b.json]) [{ file: b.json, text: null }]
}

@test
def "subjects list commits after tag that touch paths oldest first" [] {
    let context = $in
    put $context.dir crates/a.rs 0 | ignore
    commit $context.dir 'feat: before'
    git-in $context.dir tag v0.1.0 | ignore
    put $context.dir crates/a.rs 1 | ignore
    commit $context.dir 'feat: first'
    put $context.dir docs/a.md 2 | ignore
    commit $context.dir 'docs: elsewhere'
    put $context.dir crates/b.rs 3 | ignore
    commit $context.dir 'fix: second'

    let actual = with-git { subjects $context.dir v0.1.0 [crates] }

    assert equal $actual ['feat: first', 'fix: second']
}

@test
def "subjects leave out merge commits" [] {
    let context = $in
    commit $context.dir 'init: a'
    git-in $context.dir tag v0.1.0 | ignore
    git-in $context.dir switch --quiet --create topic | ignore
    put $context.dir crates/a.rs 1 | ignore
    commit $context.dir 'feat: topic'
    git-in $context.dir switch --quiet main | ignore
    put $context.dir crates/b.rs 2 | ignore
    commit $context.dir 'fix: main'
    git-in $context.dir merge --quiet --no-ff --message 'merge topic' topic | ignore

    let actual = with-git { subjects $context.dir v0.1.0 [crates] }

    assert equal ($actual | sort) ['feat: topic', 'fix: main']
}

@test
def "create-tag makes annotated tag named in its message" [] {
    let context = $in
    commit $context.dir 'init: a'

    with-git { create-tag $context.dir v0.1.0 }

    let message = git-in $context.dir tag --list --format '%(contents:subject)' v0.1.0 | str trim
    assert equal $message 'Release v0.1.0'
}

@test
def "update-locks writes version of manifest into lock" [] {
    let context = $in
    let manifest = "[package]\nname = \"a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n"
    put $context.dir a/Cargo.toml $manifest | ignore
    put $context.dir a/src/lib.rs '' | ignore
    ^cargo generate-lockfile --offline --quiet --manifest-path (
        $context.dir | path join a/Cargo.toml
    )
    put $context.dir a/Cargo.toml ($manifest | str replace 0.1.0 0.2.0) | ignore

    update-locks $context.dir [a/Cargo.lock]

    assert equal (places locked (open --raw ($context.dir | path join a/Cargo.lock))) [0.2.0]
}
