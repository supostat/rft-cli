use crate::error::Result;

pub async fn run(indices: Vec<usize>) -> Result<()> {
    super::stop::run(indices.clone()).await?;
    super::start::run(indices).await?;
    Ok(())
}
