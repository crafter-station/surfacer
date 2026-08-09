use crate::cli::EmitArgs;

pub async fn run(args: EmitArgs) -> anyhow::Result<()> {
    crate::emit_command(args).await?;
    Ok(())
}
