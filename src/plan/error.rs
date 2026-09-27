#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("")]
    SrcDirDstNot,
    #[error("")]
    DstDirSrcNot,
}
