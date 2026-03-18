use std::io;

fn fallible() -> Result<(), io::Error> {
    Ok(())
}

fn maybe() -> Option<i32> {
    Some(42)
}

// r[verify lint.anyhow-missing-context.anyhow-only]
fn bare_question_mark_non_anyhow() -> Result<(), io::Error> {
    fallible()?;
    Ok(())
}

// r[verify lint.anyhow-missing-context.option-ignored]
fn option_question_mark() -> Option<i32> {
    let val = maybe()?;
    Some(val)
}

fn main() {}
