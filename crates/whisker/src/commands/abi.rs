use clawless::prelude::*;

use crate::custom_lints::AbiTag;

/// Print the tag that names the prebuilt lints this whisker can load
#[derive(Debug, Args)]
pub struct AbiArgs {}

#[command]
pub async fn abi(_args: AbiArgs, _context: Context) -> CommandResult {
    println!("{}", AbiTag::host());

    Ok(())
}
