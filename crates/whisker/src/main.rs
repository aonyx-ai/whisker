mod commands;
mod config;
mod custom_lints;
mod discovery;

use clawless::clap;
use clawless::output::OutputFlags;
use clawless::resolved_leaf::ResolvedLeaf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = OutputFlags::augment_command(commands::clawless_init())
        .name("whisker")
        .version(env!("CARGO_PKG_VERSION"));
    let app = with_subcommands_in_order(app);

    match commands::clawless_resolve(app.get_matches()) {
        ResolvedLeaf::Command { matches, exec } => {
            clawless::runner::CommandRunner::run(matches, exec)
        }
        ResolvedLeaf::Application { matches, exec } => {
            clawless::tui::runner::ApplicationRunner::run(matches, exec)
        }
    }
}

/// Returns `app` with its subcommands listed in a stable order
///
/// Clap lists subcommands in the order it receives them. Clawless hands
/// them over in the order the linker laid out its registry. That order
/// holds for one binary and can differ for the next, so two builds of
/// whisker could print their commands in two orders.
///
/// One rank for every subcommand leaves clap to sort them by name. A
/// reader looks for a command under its name, so that order also helps.
fn with_subcommands_in_order(app: clap::Command) -> clap::Command {
    let names: Vec<String> = app
        .get_subcommands()
        .map(|subcommand| subcommand.get_name().to_owned())
        .collect();

    names.into_iter().fold(app, |app, name| {
        app.mut_subcommand(name, |subcommand| subcommand.display_order(0))
    })
}
