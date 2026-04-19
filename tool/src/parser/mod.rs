pub mod generic;
 
use crate::ir::{Edge, Node};
use std::path::Path;
use anyhow::Result;
 
pub trait CodeParser: Sync + Send {
    fn parse(&self, file_path: &Path, content: &str) -> Result<(Vec<Node>, Vec<Edge>)>;
}
