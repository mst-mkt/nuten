use std/assert
use std/testing *

source ../lib/version.nu

@test
def "bump resets parts below bumped one" [] {
    let actual = [patch, minor, major] | each { bump 1.2.3 $in }

    assert equal $actual [1.2.4, 1.3.0, 2.0.0]
}

@test
def "bump rejects unknown part" [] {
    let message = try { bump 1.2.3 pre } catch {|error| $error.msg }

    assert str contains $message 'pre is not one of major, minor, patch'
}

@test
def "bump rejects version with leading zero" [] {
    let message = try { bump 01.2.3 patch } catch {|error| $error.msg }

    assert str contains $message '01.2.3 is not a version'
}

@test
def "newer compares each part as number" [] {
    assert (newer 0.10.0 0.9.0)
    assert (newer 1.0.0 0.99.99)
    assert not (newer 0.1.9 0.2.0)
}

@test
def "newer compares patch when major and minor are equal" [] {
    assert (newer 0.1.2 0.1.1)
    assert not (newer 0.1.1 0.1.2)
}

@test
def "newer does not hold for equal versions" [] {
    assert not (newer 0.1.0 0.1.0)
}

@test
def "newest picks newest version" [] {
    assert equal ([0.2.0, 0.10.0, 0.9.0] | newest) 0.10.0
}

@test
def "newest is nothing for no versions" [] {
    assert equal ([] | newest) null
}

@test
def "fill puts version in place of placeholder" [] {
    assert equal (fill 'b/v{version}' 1.2.3) b/v1.2.3
}

@test
def "extract takes version from filled template" [] {
    assert equal (extract 'b/v{version}' b/v1.2.3) 1.2.3
}

@test
def "extract takes version from template with text after placeholder" [] {
    assert equal (extract 'a{version}z' a1.2.3z) 1.2.3
}

@test
def "extract finds nothing in text that does not fill template" [] {
    let texts = [vb/v1.2.3, b/1.2.3, b/v1.2, b/v1.2.3-rc.1, b/v01.2.3, b/v1.02.3, b/v1.2.03, a1.2.3]

    let actual = $texts | where { (extract 'b/v{version}' $in) != null }

    assert equal $actual []
}

@test
def "extract finds nothing when text after placeholder differs" [] {
    assert equal ([a1.2.3, a1.2.3y] | where { (extract 'a{version}z' $in) != null }) []
}
