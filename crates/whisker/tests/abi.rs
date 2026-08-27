use assert_cmd::Command;
use predicates::prelude::*;

/// Pins the shape of the tag the command prints
///
/// The digest is sixteen hexadecimal digits, and the platform follows it.
/// Publishers put this string in an archive name, so the shape is a
/// contract.
#[test]
fn abi_prints_a_digest_and_the_target_triple() {
    let mut command = Command::cargo_bin("whisker").expect("whisker binary should exist");

    let assertion = command.arg("abi").assert();

    assertion.success().stdout(
        predicate::str::is_match(r"^[0-9a-f]{16}-\S+\n$").expect("the pattern should compile"),
    );
}

/// The build script bakes the target into the binary, and it bakes the
/// same value into this test. The test therefore compares two values and
/// describes neither.
#[test]
fn abi_names_the_target_the_binary_was_built_for() {
    let mut command = Command::cargo_bin("whisker").expect("whisker binary should exist");

    let assertion = command.arg("abi").assert();

    assertion
        .success()
        .stdout(predicate::str::ends_with(format!(
            "-{}\n",
            env!("WHISKER_TARGET")
        )));
}
