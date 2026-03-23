use sl_core::FormItem;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MacroValue {
    Nil,
    Bool(bool),
    Int(i64),
    String(String),
    #[allow(dead_code)]
    Expr(String),
    AstItems(Vec<FormItem>),
    Keyword(Vec<(String, MacroValue)>),
    /// List of values — preserves list structure across CtValue/MacroValue bridge.
    List(Vec<MacroValue>),
}
