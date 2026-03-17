use std::io::ErrorKind;

enum Status {
    Active,
    Inactive,
    Pending,
}

#[non_exhaustive]
enum LocalNonExhaustive {
    A,
    B,
}

fn explicit_match(status: Status) {
    match status {
        Status::Active => {},
        Status::Inactive => {},
        Status::Pending => {},
    }
}

// r[verify lint.wildcard-match-arm.detect]
// r[verify lint.wildcard-match-arm.level]
// r[verify lint.wildcard-match-arm.message]
fn wildcard_match(status: Status) {
    match status {
        Status::Active => {},
        _ => {}, //~ ERROR wildcard match arm hides unhandled variants
    }
}

// r[verify lint.wildcard-match-arm.non-exhaustive-local]
fn local_non_exhaustive_wildcard(value: LocalNonExhaustive) {
    match value {
        LocalNonExhaustive::A => {},
        _ => {}, //~ ERROR wildcard match arm hides unhandled variants
    }
}

// r[verify lint.wildcard-match-arm.non-exhaustive-external]
fn external_non_exhaustive_wildcard(kind: ErrorKind) {
    match kind {
        ErrorKind::NotFound => {},
        _ => {},
    }
}

// r[verify lint.wildcard-match-arm.non-enum-types]
fn integer_wildcard(n: i32) {
    match n {
        0 => {},
        1 => {},
        _ => {},
    }
}

// r[verify lint.wildcard-match-arm.non-enum-types]
fn bool_match(b: bool) {
    match b {
        true => {},
        _ => {},
    }
}

// r[verify lint.wildcard-match-arm.detect]
fn if_let_ignored(status: Status) {
    if let Status::Active = status {}
}

fn main() {}
