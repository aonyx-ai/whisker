// r[verify lint.bool-param.detect-fn]
// r[verify lint.bool-param.level]
// r[verify lint.bool-param.message]
fn create_repo(name: &str, is_public: bool) {
    //~^ ERROR parameter has type `bool`
    let _ = name;
    let _ = is_public;
}

// r[verify lint.bool-param.detect-fn]
fn multiple_bools(a: bool, b: bool) {
    //~^ ERROR parameter has type `bool`
    //~| ERROR parameter has type `bool`
    let _ = a;
    let _ = b;
}

// r[verify lint.bool-param.detect-struct]
// r[verify lint.bool-param.message]
struct Config {
    verbose: bool,  //~ ERROR struct field has type `bool`
    name: String,
    debug: bool,    //~ ERROR struct field has type `bool`
}

// r[verify lint.bool-param.return-type-allowed]
fn returns_bool() -> bool {
    true
}

// r[verify lint.bool-param.return-type-allowed]
fn takes_str_returns_bool(s: &str) -> bool {
    s.is_empty()
}

// r[verify lint.bool-param.local-var-allowed]
fn uses_bool_locally() {
    let flag: bool = true;
    let _ = flag;
}

// r[verify lint.bool-param.detect-fn]
fn single_bool_param(flag: bool) {
    //~^ ERROR parameter has type `bool`
    let _ = flag;
}

struct NoBoolFields {
    count: i32,
    name: String,
}

fn no_params() {}

fn main() {}
