use crate::cli::InstallArgs;

pub async fn run(args: InstallArgs) -> anyhow::Result<()> {
    crate::install_command(args).await?;
    Ok(())
}
