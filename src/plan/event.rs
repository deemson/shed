use super::error::Error;

#[derive(Debug)]
pub enum Event {
    Error { error: Error },
    Done,
}
