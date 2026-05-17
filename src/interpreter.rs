use crate::ast::*;
use crate::environment::Environment;
use crate::value::Value;

const RETURN_PREFIX: &str = "__return__";

pub struct Interpreter {
    env: Environment,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            env: Environment::new(),
        }
    }

    pub fn interpret(&mut self, program: &Program) -> Result<(), String> {
        for stmt in &program.statements {
            self.execute(stmt)?;
        }
        Ok(())
    }

    fn execute(&mut self, stmt: &Stmt) -> Result<Value, String> {
        match stmt {
            Stmt::VarDecl(name, expr) => {
                let value = self.evaluate(expr)?;
                self.env.define(name, value);
                Ok(Value::Null)
            }
            Stmt::FnDecl(name, params, body) => {
                let func = Value::Function(params.clone(), body.clone());
                self.env.define(name, func);
                Ok(Value::Null)
            }
            Stmt::ExprStmt(expr) => {
                self.evaluate(expr)?;
                Ok(Value::Null)
            }
            Stmt::Return(expr) => {
                let value = match expr {
                    Some(e) => self.evaluate(e)?,
                    None => Value::Null,
                };
                Err(format!("{}{:?}", RETURN_PREFIX, value))
            }
            Stmt::Block(stmts) => {
                self.execute_block(stmts, Environment::new_enclosed(self.env.clone()))
            }
        }
    }

    // 修正点：去掉 mut 关键字
    fn execute_block(
        &mut self,
        stmts: &[Stmt],
        new_env: Environment,
    ) -> Result<Value, String> {
        let old_env = std::mem::replace(&mut self.env, new_env);
        let mut result = Value::Null;

        for stmt in stmts {
            match self.execute(stmt) {
                Ok(val) => result = val,
                Err(e) => {
                    if e.starts_with(RETURN_PREFIX) {
                        self.env = old_env;
                        return Err(e);
                    } else {
                        self.env = old_env;
                        return Err(e);
                    }
                }
            }
        }

        self.env = old_env;
        Ok(result)
    }

    fn evaluate(&mut self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::IntLit(n) => Ok(Value::Int(*n)),
            Expr::StrLit(s) => Ok(Value::Str(s.clone())),
            Expr::BoolLit(b) => Ok(Value::Bool(*b)),
            Expr::Ident(name) => self.env.get(name),
            Expr::Binary(left, op, right) => {
                let left_val = self.evaluate(left)?;
                let right_val = self.evaluate(right)?;
                match op {
                    BinOp::Add => match (left_val, right_val) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                        (Value::Str(a), Value::Str(b)) => {
                            Ok(Value::Str(format!("{}{}", a, b)))
                        }
                        _ => Err("加法操作类型不匹配".to_string()),
                    },
                    BinOp::Sub => int_op(left_val, right_val, |a, b| a - b),
                    BinOp::Mul => int_op(left_val, right_val, |a, b| a * b),
                    BinOp::Div => int_op(left_val, right_val, |a, b| a / b),
                    BinOp::Lt => cmp_op(left_val, right_val, |a, b| a < b),
                    BinOp::Gt => cmp_op(left_val, right_val, |a, b| a > b),
                    BinOp::Le => cmp_op(left_val, right_val, |a, b| a <= b),
                    BinOp::Ge => cmp_op(left_val, right_val, |a, b| a >= b),
                    BinOp::EqEq => Ok(Value::Bool(left_val == right_val)),
                    BinOp::Neq => Ok(Value::Bool(left_val != right_val)),
                    BinOp::And => {
                        Ok(Value::Bool(is_truthy(&left_val) && is_truthy(&right_val)))
                    }
                    BinOp::Or => {
                        Ok(Value::Bool(is_truthy(&left_val) || is_truthy(&right_val)))
                    }
                }
            }
            Expr::Unary(op, expr) => {
                let val = self.evaluate(expr)?;
                match op {
                    UnaryOp::Minus => match val {
                        Value::Int(n) => Ok(Value::Int(-n)),
                        _ => Err("取负操作要求整数".to_string()),
                    },
                    UnaryOp::Not => Ok(Value::Bool(!is_truthy(&val))),
                }
            }
            Expr::Call(name, args) => {
                let func = self.env.get(name)?;
                let evaluated_args: Vec<Value> = args
                    .iter()
                    .map(|a| self.evaluate(a))
                    .collect::<Result<_, _>>()?;

                match func {
                    Value::Function(params, body) => {
                        if params.len() != evaluated_args.len() {
                            return Err(format!(
                                "函数 '{}' 期望 {} 个参数，但传入了 {} 个",
                                name,
                                params.len(),
                                evaluated_args.len()
                            ));
                        }
                        let mut func_env = Environment::new_enclosed(self.env.clone());
                        for (param, arg) in params.iter().zip(evaluated_args.into_iter()) {
                            func_env.define(param, arg);
                        }
                        match self.execute_block(&body, func_env) {
                            Ok(_) => Ok(Value::Null),
                            Err(e) => {
                                if e.starts_with(RETURN_PREFIX) {
                                    let val_str = &e[RETURN_PREFIX.len()..];
                                    Ok(parse_return_value(val_str))
                                } else {
                                    Err(e)
                                }
                            }
                        }
                    }
                    Value::NativeFunction(f) => f(&evaluated_args),
                    _ => Err(format!("'{}' 不是一个函数", name)),
                }
            }
        }
    }
}

fn int_op(a: Value, b: Value, op: fn(i64, i64) -> i64) -> Result<Value, String> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(op(a, b))),
        _ => Err("算术操作需要整数".to_string()),
    }
}

fn cmp_op(a: Value, b: Value, op: fn(i64, i64) -> bool) -> Result<Value, String> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(op(a, b))),
        _ => Err("比较操作需要整数".to_string()),
    }
}

fn is_truthy(val: &Value) -> bool {
    match val {
        Value::Bool(false) | Value::Null => false,
        Value::Int(0) => false,
        Value::Str(s) if s.is_empty() => false,
        _ => true,
    }
}

fn parse_return_value(s: &str) -> Value {
    if s.starts_with("Int(") {
        let num = s.trim_start_matches("Int(").trim_end_matches(')');
        Value::Int(num.parse().unwrap_or(0))
    } else if s.starts_with("Str(") {
        let inner = s.trim_start_matches("Str(\"").trim_end_matches("\")");
        Value::Str(inner.to_string())
    } else if s == "Null" {
        Value::Null
    } else if s == "Bool(true)" {
        Value::Bool(true)
    } else if s == "Bool(false)" {
        Value::Bool(false)
    } else {
        Value::Null
    }
}