use anyhow::Context as _;

pub enum Color {
    Red,
    Green,
    Blue,
}

pub struct Palette {
    pub primary: Color,
}

pub struct Loader {
    pub path: String,
}

impl Loader {
    pub fn load(&self) -> anyhow::Result<String> {
        std::fs::read_to_string(&self.path).context("reading the configured path")
    }
}

pub fn pick_color() -> Color {
    Color::Blue
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

pub fn match_on_field_expression(palette: Palette) {
    match palette.primary {
        Color::Red => {}
        _ => {}
    }
}

pub fn match_on_call_expression() {
    match pick_color() {
        Color::Red => {}
        _ => {}
    }
}

pub fn returns_anyhow_result() -> anyhow::Result<()> {
    let _file = std::fs::read_to_string("anyhow_bare.txt")?;
    let _with_context =
        std::fs::read_to_string("anyhow_with_context.txt").context("reading other")?;
    Ok(())
}

pub fn returns_io_result() -> std::io::Result<()> {
    let _file = std::fs::read_to_string("io_bare.txt")?;
    Ok(())
}

pub fn try_on_method_call(loader: &Loader) -> anyhow::Result<usize> {
    let contents = loader.load()?;
    Ok(contents.len())
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
