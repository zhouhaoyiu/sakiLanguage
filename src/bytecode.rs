use crate::ast::*;

#[derive(Debug, Clone)]
pub(crate) enum Instr {
    LoadInt(i64),
    LoadStr(String),
    LoadBool(bool),
    LoadNull,
    LoadUndefined,
    LoadVar(String),
    Define(String, VarKind),
    Store(String),
    Unary(UnaryOp),
    Binary(BinOp),
    MakeObject(Vec<String>),
    GetProperty { site: usize, name: String },
    Pop,
}

pub(crate) fn compile_program(program: &Program) -> Option<Vec<Instr>> {
    let mut code = Vec::new();
    for stmt in &program.statements {
        compile_stmt(stmt, &mut code)?;
    }
    Some(code)
}

fn compile_stmt(stmt: &Stmt, code: &mut Vec<Instr>) -> Option<()> {
    match stmt {
        Stmt::VarDecl(name, expr, kind) => {
            compile_expr(expr, code)?;
            code.push(Instr::Define(name.clone(), kind.clone()));
            Some(())
        }
        Stmt::ExprStmt(expr) => {
            compile_expr(expr, code)?;
            code.push(Instr::Pop);
            Some(())
        }
        _ => None,
    }
}

fn compile_expr(expr: &Expr, code: &mut Vec<Instr>) -> Option<()> {
    match expr {
        Expr::IntLit(n) => code.push(Instr::LoadInt(*n)),
        Expr::StrLit(s) => code.push(Instr::LoadStr(s.clone())),
        Expr::BoolLit(b) => code.push(Instr::LoadBool(*b)),
        Expr::NullLit => code.push(Instr::LoadNull),
        Expr::UndefinedLit => code.push(Instr::LoadUndefined),
        Expr::Ident(name) => code.push(Instr::LoadVar(name.clone())),
        Expr::Assign(name, value) => {
            compile_expr(value, code)?;
            code.push(Instr::Store(name.clone()));
        }
        Expr::Unary(op, value) => {
            compile_expr(value, code)?;
            code.push(Instr::Unary(op.clone()));
        }
        Expr::Binary(left, BinOp::And | BinOp::Or, right) => {
            let _ = (left, right);
            return None;
        }
        Expr::Binary(left, op, right) => {
            compile_expr(left, code)?;
            compile_expr(right, code)?;
            code.push(Instr::Binary(op.clone()));
        }
        Expr::Object(entries) => {
            let mut keys = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                keys.push(key.clone());
                compile_expr(value, code)?;
            }
            code.push(Instr::MakeObject(keys));
        }
        Expr::Property(object, name) => {
            compile_expr(object, code)?;
            code.push(Instr::GetProperty {
                site: expr as *const Expr as usize,
                name: name.clone(),
            });
        }
        _ => return None,
    }
    Some(())
}
