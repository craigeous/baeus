// Tree-sitter syntax highlighting
//
// Note: Full tree-sitter integration for YAML highlighting requires
// initializing the tree-sitter parser with the YAML grammar and mapping
// AST node types to color tokens. This module provides the token types
// and highlight configuration that will be used with tree-sitter.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HighlightToken {
    Key,
    StringValue,
    NumberValue,
    BooleanValue,
    NullValue,
    Comment,
    Punctuation,
    Tag,
    Anchor,
    Alias,
    Default,
}

impl HighlightToken {
    pub fn from_node_kind(kind: &str) -> Self {
        match kind {
            "block_mapping_pair" | "flow_pair" => Self::Key,
            "double_quote_scalar" | "single_quote_scalar" | "block_scalar" => Self::StringValue,
            "integer_scalar" | "float_scalar" => Self::NumberValue,
            "boolean_scalar" => Self::BooleanValue,
            "null_scalar" => Self::NullValue,
            "comment" => Self::Comment,
            "tag" | "tag_handle" | "tag_prefix" => Self::Tag,
            "anchor" => Self::Anchor,
            "alias" => Self::Alias,
            _ => Self::Default,
        }
    }

    pub fn is_value(&self) -> bool {
        matches!(self, Self::StringValue | Self::NumberValue | Self::BooleanValue | Self::NullValue)
    }
}

#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub token: HighlightToken,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_from_node_kind() {
        assert_eq!(HighlightToken::from_node_kind("block_mapping_pair"), HighlightToken::Key);
        assert_eq!(
            HighlightToken::from_node_kind("double_quote_scalar"),
            HighlightToken::StringValue
        );
        assert_eq!(HighlightToken::from_node_kind("integer_scalar"), HighlightToken::NumberValue);
        assert_eq!(HighlightToken::from_node_kind("boolean_scalar"), HighlightToken::BooleanValue);
        assert_eq!(HighlightToken::from_node_kind("null_scalar"), HighlightToken::NullValue);
        assert_eq!(HighlightToken::from_node_kind("comment"), HighlightToken::Comment);
        assert_eq!(HighlightToken::from_node_kind("unknown"), HighlightToken::Default);
    }

    #[test]
    fn test_token_is_value() {
        assert!(HighlightToken::StringValue.is_value());
        assert!(HighlightToken::NumberValue.is_value());
        assert!(HighlightToken::BooleanValue.is_value());
        assert!(HighlightToken::NullValue.is_value());
        assert!(!HighlightToken::Key.is_value());
        assert!(!HighlightToken::Comment.is_value());
    }
}
