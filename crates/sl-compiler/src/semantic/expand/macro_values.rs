use sl_core::FormItem;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MacroValue {
    String(String),
    Expr(String),
    AstItems(Vec<FormItem>),
    Bool(bool),
    Int(i64),
}
