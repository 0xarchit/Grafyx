pub mod js;

use crate::ir::{Edge, Node};
use std::path::Path;

pub trait CodeParser: Sync + Send {
    fn parse(&self, file_path: &Path, content: &str) -> (Vec<Node>, Vec<Edge>);
}
