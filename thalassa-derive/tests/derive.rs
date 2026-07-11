#[test]
fn derive() {
    let t = trybuild::TestCases::new();
    t.pass("tests/derive/pass.*.rs");
    t.compile_fail("tests/derive/fail.*.rs");
}
