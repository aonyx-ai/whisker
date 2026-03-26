#![feature(rustc_private)]

mod commands;
mod driver;
pub(crate) mod toolchain;

// r[impl driver.mode-detection]
// r[impl cli.version]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("__WHISKER_DRIVER").is_ok() {
        driver::run();
        return Ok(());
    }

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

    Ok(())
}
