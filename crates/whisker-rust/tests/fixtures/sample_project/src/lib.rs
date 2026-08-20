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

pub mod errors {
    #[derive(Debug)]
    pub struct Error;
}

pub mod myres {
    pub enum Result<T> {
        Ok(T),
    }
}

pub struct Rendered;

impl std::fmt::Display for Rendered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rendered")?;
        Ok(())
    }
}

pub fn returns_syn_result() -> syn::Result<()> {
    let _ident: syn::Ident = syn::parse_str("whisker")?;
    Ok(())
}

pub fn returns_local_error_result() -> Result<(), errors::Error> {
    inner_local()?;
    Ok(())
}

fn inner_local() -> Result<(), errors::Error> {
    Ok(())
}

pub fn returns_boxed_error() -> Result<(), Box<dyn std::error::Error>> {
    let _file = std::fs::read_to_string("boxed.txt")?;
    Ok(())
}

pub fn returns_generic_error<E>() -> Result<(), E>
where
    E: From<std::io::Error>,
{
    let _file = std::fs::read_to_string("generic.txt")?;
    Ok(())
}

pub async fn returns_anyhow_result_async() -> anyhow::Result<()> {
    let _file = std::fs::read_to_string("async_anyhow_bare.txt")?;
    Ok(())
}

pub async fn returns_io_result_async() -> std::io::Result<()> {
    let _file = std::fs::read_to_string("async_io_bare.txt")?;
    Ok(())
}

pub trait AsyncLoad {
    fn load_async(&self) -> impl std::future::Future<Output = anyhow::Result<()>>;
}

impl AsyncLoad for Loader {
    async fn load_async(&self) -> anyhow::Result<()> {
        let _file = std::fs::read_to_string(&self.path)?;
        Ok(())
    }
}

pub fn closure_returning_io_result() -> anyhow::Result<()> {
    let read = || -> std::io::Result<()> {
        let _file = std::fs::read_to_string("closure.txt")?;
        Ok(())
    };
    read()?;
    Ok(())
}

pub fn user_defined_result() -> myres::Result<()> {
    myres::Result::Ok(())
}

#[allow(non_snake_case)]
pub mod Shapes {
    pub fn draw() {}
}

pub fn import_enum_variants(color: Color) -> bool {
    use Color::{Green, Red};

    match color {
        Red => true,
        Green => false,
        Color::Blue => false,
    }
}

pub fn import_enum_variants_by_glob(color: Color) -> bool {
    use Color::*;

    match color {
        Red => true,
        Green | Blue => false,
    }
}

pub fn import_from_module() -> usize {
    use std::collections::HashMap;

    HashMap::<u8, u8>::new().len()
}

pub fn import_from_uppercase_module() {
    use Shapes::draw;

    draw();
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
