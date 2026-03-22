use std::collections::BTreeSet;

use rhai::{AST, Engine as RhaiEngine, Expr, Stmt};
use sl_core::ScriptLangError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExprAnalysis {
    pub(crate) referenced_vars: Vec<String>,
    pub(crate) reads_external: bool,
    pub(crate) writes_external: bool,
    pub(crate) is_simple_var_ref: Option<String>,
}

impl ExprAnalysis {
    fn from_parts(
        referenced_vars: BTreeSet<String>,
        reads_external: bool,
        writes_external: bool,
        is_simple_var_ref: Option<String>,
    ) -> Self {
        Self {
            referenced_vars: referenced_vars.into_iter().collect(),
            reads_external,
            writes_external,
            is_simple_var_ref,
        }
    }
}

pub(crate) fn analyze_compiled_expr(
    source: &str,
    known_vars: &BTreeSet<String>,
) -> Result<ExprAnalysis, ScriptLangError> {
    let ast = build_analysis_engine().compile(source)?;
    analyze_ast(&ast, known_vars)
}

fn build_analysis_engine() -> RhaiEngine {
    RhaiEngine::new()
}

fn analyze_ast(ast: &AST, known_vars: &BTreeSet<String>) -> Result<ExprAnalysis, ScriptLangError> {
    let mut analyzer = Analyzer::new(known_vars);
    for stmt in ast.statements() {
        analyzer.analyze_stmt(stmt);
    }
    Ok(ExprAnalysis::from_parts(
        analyzer.referenced_vars,
        analyzer.reads_external,
        analyzer.writes_external,
        simple_var_ref(ast, known_vars),
    ))
}

fn simple_var_ref(ast: &AST, known_vars: &BTreeSet<String>) -> Option<String> {
    let [stmt] = ast.statements() else {
        return None;
    };
    let Stmt::Expr(expr) = stmt else {
        return None;
    };
    match &**expr {
        Expr::Variable(x, ..) => {
            if !x.2.is_empty() {
                return None;
            }
            let name = x.1.to_string();
            known_vars.contains(&name).then_some(name)
        }
        _ => None,
    }
}

struct Analyzer<'a> {
    known_vars: &'a BTreeSet<String>,
    scopes: Vec<BTreeSet<String>>,
    referenced_vars: BTreeSet<String>,
    reads_external: bool,
    writes_external: bool,
}

impl<'a> Analyzer<'a> {
    fn new(known_vars: &'a BTreeSet<String>) -> Self {
        Self {
            known_vars,
            scopes: vec![BTreeSet::new()],
            referenced_vars: BTreeSet::new(),
            reads_external: false,
            writes_external: false,
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(BTreeSet::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop().expect("scope stack should not be empty");
    }

    fn define_local(&mut self, name: impl Into<String>) {
        self.scopes
            .last_mut()
            .expect("scope stack should not be empty")
            .insert(name.into());
    }

    fn is_local(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }

    fn record_external_read(&mut self, name: &str) {
        if self.known_vars.contains(name) && !self.is_local(name) {
            self.referenced_vars.insert(name.to_string());
            self.reads_external = true;
        }
    }

    fn record_external_write(&mut self, name: &str) {
        if self.known_vars.contains(name) && !self.is_local(name) {
            self.referenced_vars.insert(name.to_string());
            self.writes_external = true;
        }
    }

    fn analyze_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Noop(..) => {}
            Stmt::Var(x, ..) => {
                self.analyze_expr(&x.1);
                self.define_local(x.0.as_str());
            }
            Stmt::If(x, ..) => {
                self.analyze_expr(&x.expr);
                self.analyze_block(&x.body);
                self.analyze_block(&x.branch);
            }
            Stmt::Switch(x, ..) => {
                let (expr, cases) = &**x;
                self.analyze_expr(expr);
                for expr_case in &cases.expressions {
                    self.analyze_expr(&expr_case.lhs);
                    self.analyze_expr(&expr_case.rhs);
                }
            }
            Stmt::While(x, ..) | Stmt::Do(x, ..) => {
                self.analyze_expr(&x.expr);
                self.analyze_block(&x.body);
            }
            Stmt::For(x, ..) => {
                self.analyze_expr(&x.2.expr);
                self.push_scope();
                self.define_local(x.0.as_str());
                if let Some(counter) = &x.1 {
                    self.define_local(counter.as_str());
                }
                self.analyze_block(&x.2.body);
                self.pop_scope();
            }
            Stmt::Assignment(x) => {
                self.analyze_lvalue(&x.1.lhs);
                self.analyze_expr(&x.1.rhs);
                if x.0.is_op_assignment() {
                    self.analyze_assignment_read(&x.1.lhs);
                }
            }
            Stmt::FnCall(x, ..) => {
                for arg in &x.args {
                    self.analyze_expr(arg);
                }
            }
            Stmt::Block(x) => self.analyze_block(x),
            Stmt::TryCatch(x, ..) => {
                self.analyze_block(&x.body);
                self.push_scope();
                if let Expr::Variable(catch_var, ..) = &x.expr {
                    self.define_local(catch_var.1.to_string());
                }
                self.analyze_block(&x.branch);
                self.pop_scope();
            }
            Stmt::Expr(expr) => self.analyze_expr(expr),
            Stmt::BreakLoop(expr, ..) | Stmt::Return(expr, ..) => {
                if let Some(expr) = expr {
                    self.analyze_expr(expr);
                }
            }
            Stmt::Import(x, ..) => self.analyze_expr(&x.0),
            Stmt::Export(..) => {}
            Stmt::Share(..) => {}
            _ => {}
        }
    }

    fn analyze_block(&mut self, block: &rhai::StmtBlock) {
        self.push_scope();
        for stmt in block.statements() {
            self.analyze_stmt(stmt);
        }
        self.pop_scope();
    }

    fn analyze_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Variable(x, ..) => self.record_external_read(x.1.as_str()),
            Expr::InterpolatedString(items, ..) | Expr::Array(items, ..) => {
                for expr in &**items {
                    self.analyze_expr(expr);
                }
            }
            Expr::And(items, ..) | Expr::Or(items, ..) | Expr::Coalesce(items, ..) => {
                for expr in &***items {
                    self.analyze_expr(expr);
                }
            }
            Expr::Map(map, ..) => {
                for (.., expr) in &map.0 {
                    self.analyze_expr(expr);
                }
            }
            Expr::Stmt(block) => self.analyze_block(block),
            Expr::FnCall(x, ..) | Expr::MethodCall(x, ..) => {
                for arg in &x.args {
                    self.analyze_expr(arg);
                }
            }
            Expr::Dot(x, ..) | Expr::Index(x, ..) => self.analyze_expr(&x.lhs),
            Expr::DynamicConstant(..)
            | Expr::BoolConstant(..)
            | Expr::IntegerConstant(..)
            | Expr::FloatConstant(..)
            | Expr::CharConstant(..)
            | Expr::StringConstant(..)
            | Expr::Unit(..)
            | Expr::ThisPtr(..)
            | Expr::Property(..) => {}
            Expr::Custom(x, ..) => {
                for expr in &x.inputs {
                    self.analyze_expr(expr);
                }
            }
            _ => {}
        }
    }

    fn analyze_lvalue(&mut self, expr: &Expr) {
        match expr {
            Expr::Variable(x, ..) => self.record_external_write(x.1.as_str()),
            Expr::Dot(x, ..) | Expr::Index(x, ..) => self.analyze_lvalue(&x.lhs),
            Expr::ThisPtr(..) | Expr::Property(..) => {}
            _ => self.analyze_expr(expr),
        }
    }

    fn analyze_assignment_read(&mut self, expr: &Expr) {
        match expr {
            Expr::Variable(x, ..) => self.record_external_read(x.1.as_str()),
            Expr::Dot(x, ..) | Expr::Index(x, ..) => self.analyze_assignment_read(&x.lhs),
            Expr::ThisPtr(..) | Expr::Property(..) => {}
            _ => self.analyze_expr(expr),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::analyze_compiled_expr;

    fn known(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn analyze_compiled_expr_covers_reads_shadowing_and_simple_var() {
        let simple = analyze_compiled_expr("name", &known(&["name"])).expect("analysis");
        assert_eq!(simple.referenced_vars, vec!["name".to_string()]);
        assert_eq!(simple.is_simple_var_ref, Some("name".to_string()));
        assert!(simple.reads_external);
        assert!(!simple.writes_external);

        let shadowed =
            analyze_compiled_expr("let name = 1; name + answer", &known(&["name", "answer"]))
                .expect("analysis");
        assert_eq!(shadowed.referenced_vars, vec!["answer".to_string()]);
        assert_eq!(shadowed.is_simple_var_ref, None);
        assert!(shadowed.reads_external);
    }

    #[test]
    fn analyze_compiled_expr_covers_property_map_string_and_calls() {
        let property = analyze_compiled_expr(
            r#"user.name + user["name"] + #{name: value}["name"] + "name" + run(x) + invoke(fn_ref, [x])"#,
            &known(&["user", "value", "x", "fn_ref"]),
        )
        .expect("analysis");
        assert_eq!(
            property.referenced_vars,
            vec![
                "fn_ref".to_string(),
                "user".to_string(),
                "value".to_string(),
                "x".to_string(),
            ]
        );
    }

    #[test]
    fn analyze_compiled_expr_covers_assignment_blocks_loops_and_catch() {
        let assignment =
            analyze_compiled_expr("x = answer; answer += 1;", &known(&["x", "answer"]))
                .expect("analysis");
        assert_eq!(
            assignment.referenced_vars,
            vec!["answer".to_string(), "x".to_string()]
        );
        assert!(assignment.reads_external);
        assert!(assignment.writes_external);

        let block = analyze_compiled_expr(
            "if cond { let x = 1; x + answer } for x in items { x + answer } try { foo } catch (err) { err + answer }",
            &known(&["cond", "items", "answer", "foo", "err"]),
        )
        .expect("analysis");
        assert_eq!(
            block.referenced_vars,
            vec![
                "answer".to_string(),
                "cond".to_string(),
                "items".to_string()
            ]
        );
        assert!(block.reads_external);
        assert!(!block.writes_external);
    }
}
