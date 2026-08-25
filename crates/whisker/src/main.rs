mod commands;
mod config;
mod custom_lints;
mod discovery;

use clawless::output::OutputFlags;
use clawless::resolved_leaf::ResolvedLeaf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = OutputFlags::augment_command(commands::clawless_init())
        .name("whisker")
        .version(env!("CARGO_PKG_VERSION"));

    match commands::clawless_resolve(app.get_matches()) {
        ResolvedLeaf::Command { matches, exec } => {
            clawless::runner::CommandRunner::run(matches, exec)
        }
        ResolvedLeaf::Application { matches, exec } => {
            clawless::tui::runner::ApplicationRunner::run(matches, exec)
        }
    }
}
