use crate::cli::ReconArgs;

pub async fn run(args: ReconArgs) -> anyhow::Result<()> {
    crate::recon_command(args).await?;
    Ok(())
}
