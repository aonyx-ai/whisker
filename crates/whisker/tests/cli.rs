// r[verify cli.version]
// r[verify cli.check.keep-going]
// r[verify cli.check.deny-warnings]
#[test]
fn cli_tests() {
    trycmd::TestCases::new().case("tests/cmd/*.md");
}
