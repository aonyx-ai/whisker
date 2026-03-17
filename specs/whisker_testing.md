# whisker_testing

## Test harness

r[testing.ui-mode]
The test harness must run `compiletest_rs` in UI mode to diff actual
stderr against `.stderr` snapshot files.

r[testing.annotation-mode]
The test harness must run `compiletest_rs` in compile-fail mode with
`-D warnings` to check `//~` annotations in test fixtures against actual
compiler output.

r[testing.driver-setup]
The test harness must build the lint library and locate the dylint driver
before running tests.
