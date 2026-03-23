//! Compile-time AST types for the macro language.

use sl_core::FormItem;

/// A block of compile-time statements.
#[derive(Debug, Clone)]
pub struct CtBlock {
    pub stmts: Vec<CtStmt>,
}

/// A compile-time statement.
#[derive(Debug, Clone)]
pub enum CtStmt {
    /// Variable binding: `let name = expr`
    Let { name: String, value: CtExpr },
    /// Variable mutation: `set name = expr`
    Set { name: String, value: CtExpr },
    /// Conditional execution: `if cond { ... }`
    If {
        cond: CtExpr,
        then_block: CtBlock,
        else_block: Option<CtBlock>,
    },
    /// Return a value: `return expr`
    Return { value: CtExpr },
    /// Expression statement (for side effects or builtin calls)
    Expr { expr: CtExpr },
}

/// A compile-time expression.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum CtExpr {
    /// Literal value
    Literal(CtValue),
    /// Variable reference
    Var { name: String },
    /// Builtin function call
    BuiltinCall { name: String, args: Vec<CtExpr> },
    /// Quote: produce AST from compile-time value
    Quote { body: Box<CtExpr> },
    /// Unquote: splice compile-time value into AST
    Unquote { expr: Box<CtExpr> },
    /// QuoteForms: process raw form items with hygiene/splice (internal bridge)
    /// This is used by the new evaluator to handle XML template quoting.
    QuoteForms { items: Vec<sl_core::FormItem> },
}

/// A compile-time value.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum CtValue {
    Nil,
    Bool(bool),
    Int(i64),
    String(String),
    /// Ordered keyword list: preserves attribute order
    Keyword(Vec<(String, CtValue)>),
    /// List of values
    List(Vec<CtValue>),
    /// Module reference (module path)
    ModuleRef(String),
    /// AST fragment (list of form items)
    Ast(Vec<FormItem>),
    /// Caller environment (opaque reference)
    CallerEnv,
}

impl CtValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            CtValue::Nil => "nil",
            CtValue::Bool(_) => "bool",
            CtValue::Int(_) => "int",
            CtValue::String(_) => "string",
            CtValue::Keyword(_) => "keyword",
            CtValue::List(_) => "list",
            CtValue::ModuleRef(_) => "module",
            CtValue::Ast(_) => "ast",
            CtValue::CallerEnv => "caller_env",
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            CtValue::Nil => false,
            CtValue::Bool(b) => *b,
            CtValue::Int(i) => *i != 0,
            CtValue::String(s) => !s.is_empty(),
            CtValue::Keyword(kv) => !kv.is_empty(),
            CtValue::List(items) => !items.is_empty(),
            CtValue::ModuleRef(_) => true,
            CtValue::Ast(items) => !items.is_empty(),
            CtValue::CallerEnv => true,
        }
    }
}
