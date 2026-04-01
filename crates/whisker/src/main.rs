#![feature(rustc_private)]

mod commands;
mod driver;
mod mode;
pub(crate) mod toolchain;

use mode::Mode;

// r[impl driver.mode-detection]
// r[impl cli.version]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Mode::detect() {
        Mode::Driver => {
            let args: Vec<String> = std::env::args().skip(1).collect();
            driver::run(args);
        }
        Mode::Cli => {
            let cancellation = clawless::cancellation::Cancellation::new();
            let context = clawless::context::Context::try_new(cancellation.clone())?;

            let rt = clawless::tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                clawless::tokio::spawn(clawless::signal::wait_for_shutdown(cancellation));

                let app = commands::clawless_init()
                    .name("whisker")
                    .version(env!("CARGO_PKG_VERSION"));
                commands::clawless_exec(app.get_matches(), context).await
            })?;
        }
    }

    Ok(())
}
