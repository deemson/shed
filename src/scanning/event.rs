use crate::manifest::FullIndex;
use super::error::Error;

#[derive(Debug)]
pub enum Event {

    Error { index: FullIndex, error: Error },
    Done,
}
