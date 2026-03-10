//! 文档提取
//!
//! 包含 extract_doc_comment、extract_param_docs 等函数

use std::collections::BTreeMap;
use syn::{Expr, Lit, Meta};

/// 提取 doc comment（保留原始文本格式）
///
/// 功能：
/// - 保留原始文本内容，包括 Markdown 标记：**bold**, *italic*, `code`, [links](url)
/// - 支持多段落合并（空行分隔）
/// - 支持结构化注释过滤：# Parameters, # Returns, # Example
/// - 过滤 @param、@required、@param_desc 等参数标记
/// - 支持代码块识别（``` 标记）
///
/// 注意：此函数仅保留原始文本，不进行 Markdown 解析（如转换为 HTML）。
/// 如需完整的 Markdown 支持，建议使用 pulldown-cmark 等外部库。
#[inline]
pub fn extract_doc_comment(attrs: &[syn::Attribute]) -> Option<String> {
    let mut doc_lines = Vec::new();
    let mut in_code_block = false;

    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let line = lit.value();
                        let trimmed = line.trim().trim_start_matches(':').trim();

                        if trimmed.starts_with("```") {
                            in_code_block = !in_code_block;
                            doc_lines.push(trimmed.to_string());
                            continue;
                        }

                        if trimmed.starts_with("@param") {
                            continue;
                        }

                        if trimmed.starts_with("@required") {
                            continue;
                        }

                        if trimmed.starts_with("@param_desc") {
                            continue;
                        }

                        if !in_code_block
                            && trimmed.starts_with('-')
                            && trimmed.contains('`')
                            && trimmed.contains("`:")
                        {
                            continue;
                        }

                        if trimmed.starts_with('#')
                            && (trimmed.contains("Parameters")
                                || trimmed.contains("Returns")
                                || trimmed.contains("Example"))
                        {
                            continue;
                        }

                        if trimmed.is_empty() {
                            if !doc_lines.is_empty()
                                && doc_lines.last().is_some_and(|s| !s.is_empty())
                            {
                                doc_lines.push(String::new());
                            }
                        } else {
                            doc_lines.push(trimmed.to_string());
                        }
                    }
                }
            }
        }
    }

    while doc_lines.last().is_some_and(|s| s.is_empty()) {
        doc_lines.pop();
    }

    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines.join("\n"))
    }
}

/// 从 doc comment 中提取 @param 描述
/// 支持格式：
/// - /// @param param_name 描述内容
/// - /// @required - 标记参数为必需
/// - /// @param_desc 描述内容 - 为前一个参数添加描述
pub fn extract_param_docs(attrs: &[syn::Attribute]) -> BTreeMap<String, String> {
    let mut param_docs = BTreeMap::new();

    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let content = lit.value();
                        if let Some((param_name, desc)) = parse_param_doc(&content) {
                            param_docs.insert(param_name, desc);
                        }
                    }
                }
            }
        }
    }

    param_docs
}

/// 从 doc comment 中提取参数级别的属性（如 @required param_name）
pub fn extract_param_attr_from_docs(
    attrs: &[syn::Attribute],
    param_name: &str,
    attr_name: &str,
) -> bool {
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let content = lit.value();
                        let trimmed = content.trim();
                        if let Some(rest) = trimmed.strip_prefix(&format!("@{}", attr_name)) {
                            let rest = rest.trim();
                            if rest == param_name || rest.is_empty() {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

/// 从 doc comment 中提取 @param_desc param_name desc 描述
pub fn extract_param_desc_from_docs(attrs: &[syn::Attribute], param_name: &str) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let content = lit.value();
                        let trimmed = content.trim();
                        if let Some(rest) = trimmed.strip_prefix("@param_desc") {
                            let rest = rest.trim();
                            if let Some(desc) = rest.strip_prefix(param_name) {
                                return Some(desc.trim().to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// 从 doc comment 中提取 @validate param_name expression
pub fn extract_param_validate_from_docs(
    attrs: &[syn::Attribute],
    param_name: &str,
) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let content = lit.value();
                        let trimmed = content.trim();
                        if let Some(rest) = trimmed.strip_prefix("@validate") {
                            let rest = rest.trim();
                            if let Some(expr) = rest.strip_prefix(param_name) {
                                let expr = expr.trim();
                                if !expr.is_empty() {
                                    return Some(expr.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// 从 doc comment 中提取 @transform param_name expression
pub fn extract_param_transform_from_docs(
    attrs: &[syn::Attribute],
    param_name: &str,
) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let content = lit.value();
                        let trimmed = content.trim();
                        if let Some(rest) = trimmed.strip_prefix("@transform") {
                            let rest = rest.trim();
                            if let Some(expr) = rest.strip_prefix(param_name) {
                                let expr = expr.trim();
                                if !expr.is_empty() {
                                    return Some(expr.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// 从 doc comment 中提取 @validate_msg param_name "message"
pub fn extract_validate_msg_from_docs(
    attrs: &[syn::Attribute],
    param_name: &str,
) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let content = lit.value();
                        let trimmed = content.trim();
                        if let Some(rest) = trimmed.strip_prefix("@validate_msg") {
                            let rest = rest.trim();
                            if let Some(msg_part) = rest.strip_prefix(param_name) {
                                let msg_part = msg_part.trim();
                                if msg_part.starts_with('"') && msg_part.ends_with('"') {
                                    return Some(msg_part[1..msg_part.len() - 1].to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// 解析单行 @param 文档
/// 返回 (参数名，描述)
/// 支持格式：
/// - /// @param name description
/// - /// - `name`: description
pub fn parse_param_doc(line: &str) -> Option<(String, String)> {
    let line = line.trim().trim_start_matches(':').trim();

    if let Some(rest) = line.strip_prefix("@param") {
        let rest = rest.trim();
        if let Some(space_pos) = rest.find(' ') {
            let param_name = rest[..space_pos].trim().to_string();
            let desc = rest[space_pos + 1..].trim().to_string();
            if !param_name.is_empty() && !desc.is_empty() {
                return Some((param_name, desc));
            }
        } else if !rest.is_empty() {
            return Some((rest.to_string(), String::new()));
        }
    }

    if let Some(rest) = line.strip_prefix('-') {
        let rest = rest.trim();
        if let Some(stripped) = rest.strip_prefix('`') {
            if let Some(end) = stripped.find('`') {
                let param_name = stripped[..end].to_string();
                let desc = stripped[end + 1..]
                    .trim()
                    .trim_start_matches(':')
                    .trim()
                    .to_string();
                if !param_name.is_empty() && !desc.is_empty() {
                    return Some((param_name, desc));
                }
            }
        }
    }

    None
}
