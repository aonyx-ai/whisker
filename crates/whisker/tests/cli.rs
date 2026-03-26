// r[verify cli.version]
// r[verify cli.check.keep-going]
#[test]
fn cli_tests() {
    trycmd::TestCases::new().case("tests/cmd/*.md");
}
