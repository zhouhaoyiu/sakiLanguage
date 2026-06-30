use saki_lang::ast;
use saki_lang::interpreter::Interpreter;
use saki_lang::parser::Parser;
use saki_lang::value::Value;

fn parse(source: &str) -> ast::Program {
    let mut parser = Parser::new(source);
    parser.parse_program().unwrap()
}

fn run(source: &str) -> Interpreter {
    let mut interpreter = Interpreter::new();
    interpreter.interpret(&parse(source)).unwrap();
    interpreter
}

#[test]
fn assignment_updates_existing_binding() {
    let interpreter = run("ika x = 1; x = x + 1;");
    assert_eq!(interpreter.get("x").unwrap(), Value::Int(2));
}

#[test]
fn const_variable_is_readonly() {
    let mut interpreter = Interpreter::new();
    let err = interpreter
        .interpret(&parse("const x = 1; x = 2;"))
        .expect_err("expected const reassignment to fail");
    assert_eq!(err, "变量 'x' 是只读变量");
}

#[test]
fn function_returns_value() {
    let interpreter = run("fn add(a, b) { return a + b; } ika x = add(3, 4);");
    assert_eq!(interpreter.get("x").unwrap(), Value::Int(7));
}

#[test]
fn function_expression_call() {
    let interpreter = run("ika add = fn(a, b) { return a + b; }; ika x = add(1, 2);");
    assert_eq!(interpreter.get("x").unwrap(), Value::Int(3));
}

#[test]
fn closure_captures_definition_scope() {
    let interpreter = run(
        "fn make_counter() {\n            ika n = 0;\n            return fn() {\n                n = n + 1;\n                return n;\n            };\n        }\n        ika c = make_counter();\n        ika a = c();\n        ika b = c();",
    );
    assert_eq!(interpreter.get("a").unwrap(), Value::Int(1));
    assert_eq!(interpreter.get("b").unwrap(), Value::Int(2));
}

#[test]
fn array_access() {
    let interpreter = run("ika a = [1, 2, 3]; ika b = a[1];");
    assert_eq!(interpreter.get("b").unwrap(), Value::Int(2));
}

#[test]
fn null_and_undefined_runtime_values() {
    let interpreter = run("ika a = null; ika b = undefined;");
    assert_eq!(interpreter.get("a").unwrap(), Value::Null);
    assert_eq!(interpreter.get("b").unwrap(), Value::Undefined);
}

#[test]
fn short_circuit_and_skips_rhs() {
    let interpreter = run("ika x = 0; ika ok = false and (x = 1);");
    assert_eq!(interpreter.get("ok").unwrap(), Value::Bool(false));
    assert_eq!(interpreter.get("x").unwrap(), Value::Int(0));
}

#[test]
fn short_circuit_or_skips_rhs() {
    let interpreter = run("ika x = 0; ika ok = true or (x = 1);");
    assert_eq!(interpreter.get("ok").unwrap(), Value::Bool(true));
    assert_eq!(interpreter.get("x").unwrap(), Value::Int(0));
}

#[test]
fn bang_operator_negates_truthy_values() {
    let interpreter = run("let a = !false; let b = !1;");
    assert_eq!(interpreter.get("a").unwrap(), Value::Bool(true));
    assert_eq!(interpreter.get("b").unwrap(), Value::Bool(false));
}

#[test]
fn while_break_and_continue_flow() {
    let interpreter = run(
        "ika x = 0;\n            while x < 5 {\n              x = x + 1;\n              if x == 2 { continue; }\n              if x == 4 { break; }\n            }",
    );
    assert_eq!(interpreter.get("x").unwrap(), Value::Int(4));
}

#[test]
fn for_loop_runs_init_condition_update_and_continue() {
    let interpreter = run("let sum = 0;
        for (let i = 0; i < 5; i = i + 1) {
            if i == 3 { continue; }
            sum = sum + i;
        }");
    assert_eq!(interpreter.get("sum").unwrap(), Value::Int(7));
    assert_eq!(interpreter.get("i").unwrap_err(), "未定义的变量 'i'");
}

#[test]
fn continue_or_break_outside_loop_errors() {
    let mut interpreter = Interpreter::new();

    let err = interpreter
        .interpret(&parse("break;"))
        .expect_err("expected break outside loop to fail");
    assert_eq!(err, "break 不能在循环外使用");

    let err = interpreter
        .interpret(&parse("continue;"))
        .expect_err("expected continue outside loop to fail");
    assert_eq!(err, "continue 不能在循环外使用");
}

#[test]
fn break_and_continue_in_function_body_is_invalid() {
    let mut interpreter = Interpreter::new();
    let err = interpreter
        .interpret(&parse("fn bad() { break; } bad();"))
        .expect_err("expected break in function to fail");
    assert!(err == "break 不能在循环外使用" || err == "break/continue 不能在函数体外使用");
}

#[test]
fn block_scope_does_not_leak_variables() {
    let mut interpreter = Interpreter::new();
    let err = interpreter
        .interpret(&parse("ika x = 1; if true { ika y = 2; } y = 3;"))
        .expect_err("expected block-scoped variable access to fail");
    assert_eq!(err, "未定义的变量 'y'");
}

#[test]
fn runtime_error_restores_previous_environment() {
    let mut interpreter = Interpreter::new();
    let err = interpreter
        .interpret(&parse("ika x = 1; if true { ika y = 2; missing(); }"))
        .expect_err("expected missing function to fail");
    assert_eq!(err, "未定义的变量 'missing'");

    interpreter.interpret(&parse("x = x + 1;")).unwrap();
    assert_eq!(interpreter.get("x").unwrap(), Value::Int(2));
    assert_eq!(interpreter.get("y").unwrap_err(), "未定义的变量 'y'");
}

#[test]
fn top_level_return_errors() {
    let mut interpreter = Interpreter::new();
    let err = interpreter
        .interpret(&parse("return 1;"))
        .expect_err("expected top-level return to fail");
    assert_eq!(err, "return 不能在函数外使用");
}

#[test]
fn var_is_function_scoped() {
    let interpreter = run(
        "fn f() {\n            if true { var x = 3; }\n            return x;\n        }\n        ika y = f();",
    );
    assert_eq!(interpreter.get("y").unwrap(), Value::Int(3));
}

#[test]
fn modulo_supports_integers() {
    let interpreter = run("ika x = 7 % 3;");
    assert_eq!(interpreter.get("x").unwrap(), Value::Int(1));
}

#[test]
fn array_out_of_range_is_undefined() {
    let interpreter = run("ika a = [1, 2]; ika b = a[10];");
    assert_eq!(interpreter.get("b").unwrap(), Value::Undefined);
}

#[test]
fn object_property_access_returns_value_or_undefined() {
    let interpreter =
        run("let user = {name: 'saki', age: 1}; let name = user.name; let missing = user.missing;");
    assert_eq!(
        interpreter.get("name").unwrap(),
        Value::Str("saki".to_string())
    );
    assert_eq!(interpreter.get("missing").unwrap(), Value::Undefined);
}
