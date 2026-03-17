enum Status {
    Active,
    Inactive,
    Pending,
}

// r[verify lint.no-matches-macro.detect]
// r[verify lint.no-matches-macro.level]
// r[verify lint.no-matches-macro.message]
fn matches_in_let(status: &Status) {
    let _x = matches!(status, Status::Active); //~ ERROR use of `matches!` macro
}

// r[verify lint.no-matches-macro.detect]
fn matches_in_return(status: &Status) -> bool {
    matches!(status, Status::Active | Status::Pending) //~ ERROR use of `matches!` macro
}

// r[verify lint.no-matches-macro.detect]
fn explicit_match_not_flagged(status: &Status) -> bool {
    match status {
        Status::Active => true,
        Status::Inactive | Status::Pending => false,
    }
}

// r[verify lint.no-matches-macro.detect]
fn other_macros_not_flagged(value: i32) {
    assert!(value > 0);
    println!("value: {value}");
}

fn main() {}
