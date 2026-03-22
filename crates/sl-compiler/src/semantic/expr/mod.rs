mod normalize;
mod rewrite;
mod scan;
mod template;
mod types;

pub(crate) use normalize::normalize_expr_escapes;
pub(crate) use rewrite::{
    rewrite_expr_function_calls, rewrite_expr_idents, rewrite_expr_with_consts,
    rewrite_expr_with_vars, rewrite_special_literals, rewrite_template_special_literals,
    rewrite_template_with_consts, rewrite_template_with_vars,
};
pub(crate) use scan::{is_ident_continue, is_ident_start};
pub(crate) use template::parse_text_template;
pub(crate) use types::{ExprKind, SpecialTokenKind};
