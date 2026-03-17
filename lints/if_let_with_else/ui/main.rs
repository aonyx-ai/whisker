// r[verify lint.if-let-with-else.detect]
// r[verify lint.if-let-with-else.level]
// r[verify lint.if-let-with-else.message]
fn if_let_with_non_diverging_else(value: Option<i32>) -> String {
    if let Some(x) = value { //~ ERROR `if let` with `else` should be written as a `match` expression
        format!("{x}")
    } else {
        String::from("none")
    }
}

// r[verify lint.if-let-with-else.detect]
fn if_let_result_with_else(value: Result<i32, &str>) -> i32 {
    if let Ok(x) = value { //~ ERROR `if let` with `else` should be written as a `match` expression
        x
    } else {
        -1
    }
}

// r[verify lint.if-let-with-else.no-else-allowed]
fn if_let_without_else(value: Option<i32>) {
    if let Some(x) = value {
        println!("{x}");
    }
}

// r[verify lint.if-let-with-else.diverging-else-ignored]
fn if_let_with_diverging_else_return(value: Option<i32>) -> i32 {
    if let Some(x) = value {
        x
    } else {
        return -1;
    }
}

// r[verify lint.if-let-with-else.diverging-else-ignored]
fn if_let_with_diverging_else_panic(value: Option<i32>) -> i32 {
    if let Some(x) = value {
        x
    } else {
        panic!("missing value");
    }
}

fn regular_if_with_else(flag: bool) -> &'static str {
    if flag {
        "yes"
    } else {
        "no"
    }
}

fn main() {}
