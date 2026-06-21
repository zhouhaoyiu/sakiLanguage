#![allow(dead_code)]

#[path = "../src/ast.rs"]
mod ast;
#[path = "../src/token.rs"]
mod token;
#[path = "../src/lexer.rs"]
mod lexer;
#[path = "../src/parser.rs"]
mod parser;

use parser::Parser;

fn parse(source: &str) -> ast::Program {
    let mut parser = Parser::new(source);
    parser.parse_program().unwrap()
}

#[test]
fn parse_var_decl_and_assignment() {
    let program = parse("ika x = 1; x = x + 1;");
    assert_eq!(program.statements.len(), 2);

    assert!(matches!(
        &program.statements[0],
        ast::Stmt::VarDecl(name, ast::Expr::IntLit(1), ast::VarKind::Let) if name == "x"
    ));

    assert_eq!(
        program.statements[1],
        ast::Stmt::ExprStmt(ast::Expr::Assign(
            "x".to_string(),
            Box::new(ast::Expr::Binary(
                Box::new(ast::Expr::Ident("x".to_string())),
                ast::BinOp::Add,
                Box::new(ast::Expr::IntLit(1))
            ))
        ))
    );
}

#[test]
fn parse_if_else() {
    let program = parse("if false { saki(1); } else { saki(2); }");
    assert_eq!(program.statements.len(), 1);

    let ast::Stmt::If(condition, then_body, Some(else_body)) = &program.statements[0] else {
        panic!("expected if/else statement")
    };

    assert_eq!(*condition, ast::Expr::BoolLit(false));
    assert_eq!(then_body.len(), 1);
    assert!(matches!(
        &then_body[0],
        ast::Stmt::ExprStmt(ast::Expr::Call(_, _))
    ));
    assert_eq!(else_body.len(), 1);
    assert!(matches!(&else_body[0], ast::Stmt::ExprStmt(ast::Expr::Call(_, _))));
}

#[test]
fn parse_else_if() {
    let program = parse("if false { 1; } else if true { 2; } else { 3; }");
    assert_eq!(program.statements.len(), 1);

    assert!(matches!(program.statements[0], ast::Stmt::If(_, _, Some(_))));

    let ast::Stmt::If(_, _, Some(else_body)) = &program.statements[0] else {
        panic!("expected if with else branch")
    };

    assert_eq!(else_body.len(), 1);
    assert!(matches!(&else_body[0], ast::Stmt::If(_, _, Some(_))));
}

#[test]
fn parse_break_and_continue_statements() {
    let program = parse("while true { if false { continue; } break; }");
    assert_eq!(program.statements.len(), 1);

    let ast::Stmt::While(_, body) = &program.statements[0] else {
        panic!("expected while statement")
    };

    assert_eq!(body.len(), 2);
    assert!(matches!(body[0], ast::Stmt::If(_, _, None)));
    assert!(matches!(body[1], ast::Stmt::Break));
}

#[test]
fn parse_function_expression() {
    let program = parse("ika f = fn(a, b) { return a + b; };");
    assert!(matches!(
        &program.statements[0],
        ast::Stmt::VarDecl(_, ast::Expr::FunctionExpr(_, _), ast::VarKind::Let)
    ));
}

#[test]
fn parse_array_literal() {
    let program = parse("ika a = [1, 2, 3];");
    assert_eq!(program.statements.len(), 1);

    assert!(matches!(
        &program.statements[0],
        ast::Stmt::VarDecl(_, ast::Expr::Array(items), ast::VarKind::Let) if items.len() == 3
    ));
}

#[test]
fn parse_js_style_operator_aliases() {
    let program = parse("ika x = 1 === 1; ika y = 2 !== 3; ika z = 1 && 0;");
    assert_eq!(program.statements.len(), 3);
    assert!(matches!(
        &program.statements[0],
        ast::Stmt::VarDecl(_, ast::Expr::Binary(_, ast::BinOp::EqEqEq, _), ast::VarKind::Let)
    ));
    assert!(matches!(
        &program.statements[1],
        ast::Stmt::VarDecl(_, ast::Expr::Binary(_, ast::BinOp::NeqEq, _), ast::VarKind::Let)
    ));
    assert!(matches!(
        &program.statements[2],
        ast::Stmt::VarDecl(_, ast::Expr::Binary(_, ast::BinOp::And, _), ast::VarKind::Let)
    ));
}
