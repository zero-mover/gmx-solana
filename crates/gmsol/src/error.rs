/// Error type for `gmsol`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Client Error.
    #[error(transparent)]
    Client(#[from] anchor_client::ClientError),
}
