use anyhow::Context as _;

pub enum Color {
    Red,
    Green,
    Blue,
}

pub fn match_on_enum(color: Color) {
    match color {
        Color::Red => {}
        _ => {}
    }
}

pub fn match_on_integer(x: i32) {
    match x {
        0 => {}
        _ => {}
    }
}

pub fn returns_anyhow_result() -> anyhow::Result<()> {
    let _file = std::fs::read_to_string("test.txt")?;
    let _with_context = std::fs::read_to_string("other.txt").context("reading other")?;
    Ok(())
}

pub fn returns_io_result() -> std::io::Result<()> {
    let _file = std::fs::read_to_string("test.txt")?;
    Ok(())
}

pub fn if_let_with_diverging_else(x: Option<i32>) -> i32 {
    if let Some(v) = x {
        v
    } else {
        return 0;
    }
}

pub fn if_let_with_non_diverging_else(x: Option<i32>) -> i32 {
    if let Some(v) = x {
        v
    } else {
        42
    }
}

pub fn bool_param(verbose: bool) -> &'static str {
    if verbose {
        "verbose"
    } else {
        "quiet"
    }
}

pub struct Config {
    pub debug: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn match_on_enum_under_cfg_test(color: Color) {
        match color {
            Color::Red => {}
            _ => {}
        }
    }
}
