use super::HeddleVersion;

#[test]
fn parses_a_heddle_tag() {
    let v = HeddleVersion::parse("v0.3.1").expect("v0.3.1 must parse");
    assert_eq!((v.major(), v.minor(), v.patch()), (0, 3, 1));
}

#[test]
fn parses_without_the_v_prefix() {
    // CFBundleShortVersionString has no `v`; the manifest tag does. Accept both so the
    // caller never has to remember which side it is holding.
    let v = HeddleVersion::parse("0.3.1").expect("0.3.1 must parse");
    assert_eq!((v.major(), v.minor(), v.patch()), (0, 3, 1));
}

#[test]
fn orders_by_component_not_lexically() {
    let older = HeddleVersion::parse("v0.9.0").unwrap();
    let newer = HeddleVersion::parse("v0.10.0").unwrap();
    // Lexically "0.10.0" < "0.9.0"; numerically it is not. A string comparison here would
    // silently refuse every update after 0.9.
    assert!(newer > older, "0.10.0 must be newer than 0.9.0");
}

#[test]
fn orders_across_every_component() {
    // Each pair differs in exactly one component, so a comparison that ignored any one of
    // them would fail here rather than passing by luck on a single example.
    let cases = [
        ("v0.3.1", "v1.0.0"),
        ("v0.3.1", "v0.4.0"),
        ("v0.3.1", "v0.3.2"),
        ("v1.9.9", "v2.0.0"),
    ];
    for (older, newer) in cases {
        let older = HeddleVersion::parse(older).unwrap();
        let newer = HeddleVersion::parse(newer).unwrap();
        assert!(newer > older, "{newer:?} must be newer than {older:?}");
        assert!(older < newer, "ordering must be symmetric");
    }
}

#[test]
fn equal_versions_are_not_newer() {
    let a = HeddleVersion::parse("v0.3.1").unwrap();
    let b = HeddleVersion::parse("v0.3.1").unwrap();
    assert!(!(a > b) && !(b > a), "identical versions must not order");
    assert_eq!(a, b);
}

#[test]
fn accepts_the_v_prefix_on_either_side_of_a_comparison() {
    // The running version comes from CFBundleShortVersionString (no `v`) and the manifest
    // version from a git tag (with `v`). They are compared against each other constantly, so
    // the prefix must not affect the result.
    let bundle = HeddleVersion::parse("0.3.1").unwrap();
    let tag = HeddleVersion::parse("v0.3.1").unwrap();
    assert_eq!(bundle, tag);
}

#[test]
fn rejects_warps_dated_scheme() {
    // Not a guard against an attacker -- a guard against US. If a manifest ever carries
    // upstream's format, this must refuse to parse rather than produce a number that
    // happens to compare.
    assert!(HeddleVersion::parse("v0.2026.07.26.18.00.stable_01").is_none());
}

#[test]
fn rejects_junk() {
    for junk in [
        "",
        "v",
        "v1",
        "v1.2",
        "v1.2.3.4",
        "va.b.c",
        "1.2.3-beta",
        "vX.Y.Z",
        "0.3.1 ",
        " 0.3.1",
        "0..1",
        "-1.0.0",
        "0.3.1+build",
    ] {
        assert!(
            HeddleVersion::parse(junk).is_none(),
            "{junk:?} must not parse: an unparseable version has to be treated as \
             'do not update', never as version zero"
        );
    }
}

#[test]
fn junk_is_not_treated_as_version_zero() {
    // The failure mode this guards: a lenient parser returning 0.0.0 for garbage would make
    // every real running version look NEWER than the manifest, which silently disables
    // updates -- or, with the comparison the other way, offers a "downgrade" to nothing.
    assert!(HeddleVersion::parse("garbage").is_none());
    let real = HeddleVersion::parse("v0.3.1").unwrap();
    assert!(real > HeddleVersion::parse("v0.0.0").unwrap());
}
