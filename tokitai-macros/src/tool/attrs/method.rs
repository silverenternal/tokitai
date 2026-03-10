//! 方法级工具属性解析

use quote::ToTokens;
use syn::{
    parse::{Parse, ParseStream},
    token, Expr, Ident, Lit, LitStr,
};

use super::param::{
    parse_json_value, parse_lit_to_f64, parse_lit_to_string, parse_lit_to_usize, parse_value_string,
};
use crate::tool::types::param::ParamToolAttrs;

/// impl 块级别的工具属性
#[derive(Default)]
pub struct ToolAttributes {
    pub name: Option<String>,
    pub description: Option<String>,
}

impl Parse for ToolAttributes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<token::Eq>()?;

            let value: LitStr = input.parse()?;
            match key.to_string().as_str() {
                "name" => name = Some(value.value()),
                "desc" | "description" => description = Some(value.value()),
                _ => {}
            }

            if input.peek(token::Comma) {
                input.parse::<token::Comma>()?;
            }
        }

        Ok(ToolAttributes { name, description })
    }
}

/// 方法级别的工具属性
#[derive(Default)]
pub struct MethodToolAttrs {
    pub name: Option<String>,
    pub desc: Option<String>,
    pub skip: bool,
    pub deprecated: bool,
    pub replaced_by: Option<String>,
    pub deprecated_note: Option<String>,
    pub deprecated_since: Option<String>,
    pub remove_in: Option<String>,
    pub version: Option<String>,
    pub visible: bool,
    pub tags: Vec<String>,
    pub group: Option<String>,
    pub return_description: Option<String>,
    pub context: Option<String>,
    pub example_input: Option<serde_json::Value>,
    pub param_order: Option<Vec<String>>,
    pub hidden_params: Vec<String>,
    pub example_output: Option<String>,
    pub alias: Vec<String>,
    pub allow: Vec<String>,
    pub cache: Option<String>,
    pub rate_limit: Option<String>,
    pub param_validations: Vec<(String, ParamToolAttrs)>,
}

impl Parse for MethodToolAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Ident) {
            let key: Ident = input.fork().parse()?;
            if key == "skip" {
                input.parse::<Ident>()?;
                if input.peek(token::Comma) {
                    input.parse::<token::Comma>()?;
                }
                return Ok(MethodToolAttrs {
                    name: None,
                    desc: None,
                    skip: true,
                    deprecated: false,
                    replaced_by: None,
                    deprecated_note: None,
                    deprecated_since: None,
                    remove_in: None,
                    version: None,
                    visible: true,
                    tags: Vec::new(),
                    group: None,
                    return_description: None,
                    context: None,
                    example_input: None,
                    param_order: None,
                    hidden_params: Vec::new(),
                    example_output: None,
                    alias: Vec::new(),
                    allow: Vec::new(),
                    cache: None,
                    rate_limit: None,
                    param_validations: Vec::new(),
                });
            }
        }

        let mut name = None;
        let mut desc = None;
        let mut deprecated = false;
        let mut replaced_by = None;
        let mut deprecated_note = None;
        let mut deprecated_since = None;
        let mut remove_in = None;
        let mut version = None;
        let mut visible = true;
        let mut tags = Vec::new();
        let mut group = None;
        let mut return_description = None;
        let mut context = None;
        let mut example_input: Option<serde_json::Value> = None;
        let mut param_order: Option<Vec<String>> = None;
        let mut hidden_params = Vec::new();
        let mut example_output = None;
        let mut category: Option<String> = None;
        let mut alias = Vec::new();
        let mut allow = Vec::new();
        let mut cache: Option<String> = None;
        let mut rate_limit: Option<String> = None;
        let mut param_validations: Vec<(String, ParamToolAttrs)> = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;

            match key.to_string().as_str() {
                "deprecated" => {
                    if input.peek(token::Eq) {
                        input.parse::<token::Eq>()?;
                        if let Ok(lit_bool) = input.parse::<syn::LitBool>() {
                            deprecated = lit_bool.value;
                        } else {
                            deprecated = true;
                        }
                    } else {
                        deprecated = true;
                    }
                    if input.peek(token::Comma) {
                        let _ = input.parse::<token::Comma>();
                    }
                }
                "replaced_by" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    replaced_by = Some(value.value());
                }
                "deprecated_note" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    deprecated_note = Some(value.value());
                }
                "deprecated_since" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    deprecated_since = Some(value.value());
                }
                "remove_in" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    remove_in = Some(value.value());
                }
                "version" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    version = Some(value.value());
                }
                "group" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    group = Some(value.value());
                }
                "visible" => {
                    input.parse::<token::Eq>()?;
                    if let Ok(ident) = input.parse::<Ident>() {
                        visible = ident != "false";
                    } else if input.peek(LitStr) {
                        let value: LitStr = input.parse()?;
                        visible = value.value().to_lowercase() != "false";
                    } else if let Ok(Lit::Bool(lit_bool)) = input.parse::<Lit>() {
                        visible = lit_bool.value;
                    }
                }
                "tags" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let tag: LitStr = content.parse()?;
                        tags.push(tag.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                }
                "return_description" | "returns" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    return_description = Some(value.value());
                }
                "context" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    context = Some(value.value());
                }
                "example_input" | "example" => {
                    input.parse::<token::Eq>()?;
                    example_input = parse_json_value(input)?;
                }
                "param_order" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    let mut order = Vec::new();
                    while !content.is_empty() {
                        let name: LitStr = content.parse()?;
                        order.push(name.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                    param_order = Some(order);
                }
                "hidden_params" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let name: LitStr = content.parse()?;
                        hidden_params.push(name.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                }
                "example_output" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    example_output = Some(value.value());
                }
                "category" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    category = Some(value.value());
                }
                "alias" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let alias_name: LitStr = content.parse()?;
                        alias.push(alias_name.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                }
                "allow" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let warning: LitStr = content.parse()?;
                        allow.push(warning.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                }
                "cache" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    cache = Some(value.value());
                }
                "rate_limit" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    rate_limit = Some(value.value());
                }
                _ => {
                    let key_str = key.to_string();
                    let validation_prefixes = [
                        "enum_values_",
                        "min_length_",
                        "max_length_",
                        "min_items_",
                        "max_items_",
                        "multiple_of_",
                        "validate_msg_",
                        "default_",
                        "example_",
                        "one_of_",
                        "pattern_",
                        "min_",
                        "max_",
                    ];

                    let is_validation_attr = validation_prefixes
                        .iter()
                        .any(|prefix| key_str.starts_with(prefix));

                    if is_validation_attr {
                        for prefix in &validation_prefixes {
                            if key_str.starts_with(prefix) {
                                let param_name = key_str.strip_prefix(prefix).unwrap();
                                let existing_idx =
                                    param_validations.iter().position(|(n, _)| n == param_name);
                                let mut param_attrs = if let Some(idx) = existing_idx {
                                    param_validations.remove(idx).1
                                } else {
                                    ParamToolAttrs::default()
                                };

                                input.parse::<token::Eq>()?;

                                match *prefix {
                                    "one_of_" => {
                                        let content;
                                        syn::bracketed!(content in input);
                                        let mut values = Vec::new();
                                        while !content.is_empty() {
                                            let val: LitStr = content.parse()?;
                                            values.push(val.value());
                                            if content.peek(token::Comma) {
                                                content.parse::<token::Comma>()?;
                                            }
                                        }
                                        param_attrs.one_of = Some(values.clone());
                                    }
                                    "enum_values_" => {
                                        let content;
                                        syn::bracketed!(content in input);
                                        let mut values = Vec::new();
                                        while !content.is_empty() {
                                            let val_expr: Expr = content.parse()?;
                                            let val_str = val_expr.to_token_stream().to_string();
                                            values.push(parse_value_string(&val_str));
                                            if content.peek(token::Comma) {
                                                content.parse::<token::Comma>()?;
                                            }
                                        }
                                        param_attrs.enum_values = Some(values.clone());
                                    }
                                    "pattern_" => {
                                        param_attrs.pattern = parse_lit_to_string(input)?;
                                    }
                                    "min_" => {
                                        param_attrs.min = parse_lit_to_f64(input)?;
                                    }
                                    "max_" => {
                                        param_attrs.max = parse_lit_to_f64(input)?;
                                    }
                                    "min_length_" => {
                                        param_attrs.min_length = parse_lit_to_usize(input)?;
                                    }
                                    "max_length_" => {
                                        param_attrs.max_length = parse_lit_to_usize(input)?;
                                    }
                                    "min_items_" => {
                                        param_attrs.min_items = parse_lit_to_usize(input)?;
                                    }
                                    "max_items_" => {
                                        param_attrs.max_items = parse_lit_to_usize(input)?;
                                    }
                                    "multiple_of_" => {
                                        param_attrs.multiple_of = parse_lit_to_f64(input)?;
                                    }
                                    "validate_msg_" => {
                                        param_attrs.validate_msg = parse_lit_to_string(input)?;
                                    }
                                    "validate_msg_zh_" => {
                                        param_attrs.validate_msg_zh = parse_lit_to_string(input)?;
                                    }
                                    "validate_msg_en_" => {
                                        param_attrs.validate_msg_en = parse_lit_to_string(input)?;
                                    }
                                    "default_" => {
                                        param_attrs.default = parse_json_value(input)?;
                                    }
                                    "example_" => {
                                        param_attrs.example = parse_json_value(input)?;
                                    }
                                    _ => {}
                                }

                                param_validations.push((param_name.to_string(), param_attrs));
                                break;
                            }
                        }
                    } else {
                        input.parse::<token::Eq>()?;
                        let value: LitStr = input.parse()?;
                        match key.to_string().as_str() {
                            "name" => name = Some(value.value()),
                            "desc" | "description" => desc = Some(value.value()),
                            _ => {}
                        }
                    }
                }
            }

            if input.peek(token::Comma) {
                input.parse::<token::Comma>()?;
            }
        }

        if let Some(cat) = category {
            tags.push(cat);
        }

        Ok(MethodToolAttrs {
            name,
            desc,
            skip: false,
            deprecated,
            replaced_by,
            deprecated_note,
            deprecated_since,
            remove_in,
            version,
            visible,
            tags,
            group,
            return_description,
            context,
            example_input,
            param_order,
            hidden_params,
            example_output,
            alias,
            allow,
            cache,
            rate_limit,
            param_validations,
        })
    }
}
