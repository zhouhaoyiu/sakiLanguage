#![allow(dead_code)]

#[path = "../src/ast.rs"]
mod ast;
#[path = "../src/environment.rs"]
mod environment;
#[path = "../src/value.rs"]
mod value;

use environment::Environment;
use value::Value;

fn i64v(n: i64) -> Value {
    Value::Int(n)
}

#[test]
fn define_and_get_local_variable() {
    let env = Environment::new();
    env.define("x", i64v(1));
    assert_eq!(env.get("x").unwrap(), i64v(1));
}

#[test]
fn set_updates_current_scope_first() {
    let env = Environment::new();
    env.define("x", i64v(1));
    env.set("x", i64v(2)).unwrap();
    assert_eq!(env.get("x").unwrap(), i64v(2));
}

#[test]
fn set_walks_to_parent_scope() {
    let outer = Environment::new();
    outer.define("x", i64v(1));
    let inner = Environment::new_enclosed(outer);
    inner.set("x", i64v(9)).unwrap();
    assert_eq!(inner.get("x").unwrap(), i64v(9));
}

#[test]
fn var_walks_to_function_scope() {
    let outer = Environment::new();
    let inner = Environment::new_enclosed(outer.clone());
    inner.define_var("x", i64v(3));
    assert_eq!(outer.get("x").unwrap(), i64v(3));
}

#[test]
fn set_undefined_variable_returns_error() {
    let env = Environment::new();
    let err = env.set("missing", Value::Null).unwrap_err();
    assert_eq!(err, "未定义的变量 'missing'");
}
