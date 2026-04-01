#![feature(rustc_private)]

mod driver;
mod mode;
pub(crate) mod toolchain;

use mode::Mode;

// r[impl driver.mode-detection]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Mode::detect() {
        Mode::Driver => {
            let args: Vec<String> = std::env::args().skip(1).collect();
            driver::run(args);
        }
        Mode::Cli => {
            eprintln!("usage: whisker is not yet a standalone CLI tool");
            std::process::exit(1);
        }
    }

    Ok(())
}
