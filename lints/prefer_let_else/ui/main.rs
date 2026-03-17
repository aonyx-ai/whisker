fn option_value() -> Option<i32> {
    let value: Option<i32> = Some(42);

    // r[verify lint.prefer-let-else.detect]
    // r[verify lint.prefer-let-else.diverging-else]
    // r[verify lint.prefer-let-else.level]
    // r[verify lint.prefer-let-else.message]
    let x = if let Some(v) = value { //~ ERROR `if let` with a diverging `else` can be rewritten as `let-else`
        v
    } else {
        return None;
    };

    Some(x)
}

fn with_break() {
    let values = vec![Some(1), None, Some(3)];
    for item in values {
        // r[verify lint.prefer-let-else.detect]
        if let Some(v) = item { //~ ERROR `if let` with a diverging `else` can be rewritten as `let-else`
            let _ = v;
        } else {
            continue;
        }
    }
}

fn with_panic() {
    let value: Option<i32> = Some(42);

    // r[verify lint.prefer-let-else.detect]
    let _x = if let Some(v) = value { //~ ERROR `if let` with a diverging `else` can be rewritten as `let-else`
        v
    } else {
        panic!("missing value");
    };
}

// r[verify lint.prefer-let-else.if-let-only]
fn if_let_without_else() {
    let value: Option<i32> = Some(42);
    if let Some(v) = value {
        let _ = v;
    }
}

// r[verify lint.prefer-let-else.if-let-only]
fn if_let_with_non_diverging_else() {
    let value: Option<i32> = Some(42);
    if let Some(v) = value {
        let _ = v;
    } else {
        let _ = 0;
    }
}

// r[verify lint.prefer-let-else.if-let-only]
fn regular_if_else() {
    let condition = true;
    if condition {
        let _ = 1;
    } else {
        return;
    }
}

fn main() {}
