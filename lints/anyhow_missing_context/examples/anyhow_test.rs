#![allow(dead_code)]

use std::io;

use anyhow::Context;

fn fallible() -> Result<(), io::Error> {
    Ok(())
}

fn fallible_string() -> Result<String, io::Error> {
    Ok(String::new())
}

fn maybe() -> Option<i32> {
    Some(42)
}

// r[verify lint.anyhow-missing-context.detect]
// r[verify lint.anyhow-missing-context.level]
// r[verify lint.anyhow-missing-context.message]
fn bare_question_mark() -> anyhow::Result<()> {
    fallible()?; //~ ERROR use of `?` on Result without error context
    Ok(())
}

// r[verify lint.anyhow-missing-context.detect]
fn bare_question_mark_with_value() -> anyhow::Result<String> {
    let _s = fallible_string()?; //~ ERROR use of `?` on Result without error context
    Ok(String::new())
}

// r[verify lint.anyhow-missing-context.context-allowed]
fn with_context_call() -> anyhow::Result<()> {
    fallible().context("calling fallible")?;
    Ok(())
}

// r[verify lint.anyhow-missing-context.with-context-allowed]
fn with_with_context_call() -> anyhow::Result<()> {
    fallible().with_context(|| "calling fallible")?;
    Ok(())
}

// r[verify lint.anyhow-missing-context.anyhow-only]
fn non_anyhow_return() -> Result<(), io::Error> {
    fallible()?;
    Ok(())
}

// r[verify lint.anyhow-missing-context.option-ignored]
fn option_question_mark() -> Option<i32> {
    let val = maybe()?;
    Some(val)
}

fn main() {}
