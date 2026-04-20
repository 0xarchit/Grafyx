use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use anyhow::{anyhow, Error};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Root,
    Service,
    File,
    Module,
    Class,
    Function,
    Variable,
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Root => "root",
            Self::Service => "service",
            Self::File => "file",
            Self::Module => "module",
            Self::Class => "class",
            Self::Function => "function",
            Self::Variable => "variable",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for NodeKind {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "root" => Ok(Self::Root),
            "service" => Ok(Self::Service),
            "file" => Ok(Self::File),
            "module" => Ok(Self::Module),
            "class" => Ok(Self::Class),
            "function" => Ok(Self::Function),
            "variable" => Ok(Self::Variable),
            _ => Err(anyhow!("Unknown NodeKind: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum RelationType {
    RootLink,
    ServiceCall,
    Imports,
    Calls,
    Defines,
    Extends,
    Implements,
    Uses,
    ApiLink,
}

impl fmt::Display for RelationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::RootLink => "rootlink",
            Self::ServiceCall => "servicecall",
            Self::Imports => "imports",
            Self::Calls => "calls",
            Self::Defines => "defines",
            Self::Extends => "extends",
            Self::Implements => "implements",
            Self::Uses => "uses",
            Self::ApiLink => "apilink",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for RelationType {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rootlink" => Ok(Self::RootLink),
            "servicecall" => Ok(Self::ServiceCall),
            "imports" => Ok(Self::Imports),
            "calls" => Ok(Self::Calls),
            "defines" => Ok(Self::Defines),
            "extends" => Ok(Self::Extends),
            "implements" => Ok(Self::Implements),
            "uses" => Ok(Self::Uses),
            "apilink" => Ok(Self::ApiLink),
            _ => Err(anyhow!("Unknown RelationType: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    pub language: String,
    pub file_path: String,
    pub service: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Edge {
    pub from_node_id: String,
    pub to_node_id: String,
    pub relation_type: RelationType,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }
}
