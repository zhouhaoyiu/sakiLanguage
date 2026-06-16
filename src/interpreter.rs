use crate::ast::*;
use crate::environment::Environment;
use crate::value::Value;

/// 语法树解释执行器。
pub struct Interpreter {
    /// 当前执行环境。
    env: Environment,
    /// 当前循环层数。
    loop_depth: usize,
}

#[derive(Debug)]
enum ControlFlow {
    /// 普通执行结果。
    Value(Value),
    /// 函数返回。
    Return(Value),
    /// 跳出循环。
    Break,
    /// 跳过当前循环。
    Continue,
}

impl Interpreter {
    /// 创建解释器并初始化全局环境。
    pub fn new() -> Self {
        Interpreter {
            env: Environment::new(),
            loop_depth: 0,
        }
    }

    /// 执行整个程序。
    pub fn interpret(&mut self, program: &Program) -> Result<(), String> {
        for stmt in &program.statements {
            match self.execute(stmt)? {
                ControlFlow::Value(_) => {}
                ControlFlow::Return(_) => continue,
                ControlFlow::Break | ControlFlow::Continue => {
                    return Err("break/continue 不能在循环外使用".to_string())
                }
            }
        }
        Ok(())
    }

    /// 执行一条语句并返回结果。
    fn execute(&mut self, stmt: &Stmt) -> Result<ControlFlow, String> {
        match stmt {
            Stmt::VarDecl(name, expr, kind) => {
                let value = self.evaluate(expr)?;
                let mutable = !matches!(kind, VarKind::Const);
                self.env.define_with(name, value, mutable);
                Ok(ControlFlow::Value(Value::Null))
            }
            Stmt::FnDecl(name, params, body) => {
                let func = Value::Function(params.clone(), body.clone());
                self.env.define(name, func);
                Ok(ControlFlow::Value(Value::Null))
            }
            Stmt::ExprStmt(expr) => {
                self.evaluate(expr)?;
                Ok(ControlFlow::Value(Value::Null))
            }
            Stmt::Return(expr) => {
                let value = match expr {
                    Some(e) => self.evaluate(e)?,
                    None => Value::Null,
                };
                Ok(ControlFlow::Return(value))
            }
            Stmt::Break => {
                if self.loop_depth == 0 {
                    Err("break 不能在循环外使用".to_string())
                } else {
                    Ok(ControlFlow::Break)
                }
            }
            Stmt::Continue => {
                if self.loop_depth == 0 {
                    Err("continue 不能在循环外使用".to_string())
                } else {
                    Ok(ControlFlow::Continue)
                }
            }
            Stmt::If(condition, then_stmts, else_stmts) => {
                let condition = self.evaluate(condition)?;
                if is_truthy(&condition) {
                    let parent_env = std::mem::replace(&mut self.env, Environment::new());
                    self.execute_block(then_stmts, Environment::new_enclosed(parent_env))
                } else if let Some(stmts) = else_stmts {
                    let parent_env = std::mem::replace(&mut self.env, Environment::new());
                    self.execute_block(stmts, Environment::new_enclosed(parent_env))
                } else {
                    Ok(ControlFlow::Value(Value::Null))
                }
            }
            Stmt::While(condition, body) => {
                let previous_depth = self.loop_depth;
                self.loop_depth += 1;

                let result = (|| -> Result<ControlFlow, String> {
                    loop {
                        let condition = self.evaluate(condition)?;
                        if !is_truthy(&condition) {
                            break Ok(ControlFlow::Value(Value::Null));
                        }

                        let parent_env = std::mem::replace(&mut self.env, Environment::new());
                        match self.execute_block(body, Environment::new_enclosed(parent_env))? {
                            ControlFlow::Value(_) => continue,
                            ControlFlow::Return(v) => {
                                break Ok(ControlFlow::Return(v));
                            }
                            ControlFlow::Break => break Ok(ControlFlow::Value(Value::Null)),
                            ControlFlow::Continue => continue,
                        }
                    }
                })();

                self.loop_depth = previous_depth;
                result
            }
            Stmt::Block(stmts) => {
                let parent_env = std::mem::replace(&mut self.env, Environment::new());
                self.execute_block(stmts, Environment::new_enclosed(parent_env))
            }
        }
    }

    /// 执行语句块并保持作用域隔离。
    fn execute_block(
        &mut self,
        stmts: &[Stmt],
        new_env: Environment,
    ) -> Result<ControlFlow, String> {
        let _old_env = std::mem::replace(&mut self.env, new_env);
        let mut result = Value::Null;

        for stmt in stmts {
            match self.execute(stmt)? {
                ControlFlow::Value(val) => result = val,
                control => {
                    let block_env = std::mem::replace(&mut self.env, Environment::new());
                    self.env = block_env.pop_parent();
                    return Ok(control);
                }
            }
        }

        let block_env = std::mem::replace(&mut self.env, Environment::new());
        self.env = block_env.pop_parent();
        Ok(ControlFlow::Value(result))
    }

    /// 计算表达式并返回运行时值。
    fn evaluate(&mut self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::IntLit(n) => Ok(Value::Int(*n)),
            Expr::StrLit(s) => Ok(Value::Str(s.clone())),
            Expr::BoolLit(b) => Ok(Value::Bool(*b)),
            Expr::NullLit => Ok(Value::Null),
            Expr::UndefinedLit => Ok(Value::Undefined),
            Expr::Array(items) => {
                let values = items
                    .iter()
                    .map(|item| self.evaluate(item))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Value::Array(values))
            }
            Expr::Ident(name) => self.env.get(name),
            Expr::Assign(name, value) => {
                let value = self.evaluate(value)?;
                self.env.set(name, value.clone())?;
                Ok(value)
            }
            Expr::FunctionExpr(params, body) => Ok(Value::Function(params.clone(), body.clone())),
            Expr::Binary(left, op, right) => {
                let left_val = self.evaluate(left)?;
                let result = match op {
                    BinOp::And => {
                        if !is_truthy(&left_val) {
                            Value::Bool(false)
                        } else {
                            let right_val = self.evaluate(right)?;
                            Value::Bool(is_truthy(&right_val))
                        }
                    }
                    BinOp::Or => {
                        if is_truthy(&left_val) {
                            Value::Bool(true)
                        } else {
                            let right_val = self.evaluate(right)?;
                            Value::Bool(is_truthy(&right_val))
                        }
                    }
                    BinOp::Add => match (left_val, self.evaluate(right)?) {
                        (Value::Int(a), Value::Int(b)) => Value::Int(a + b),
                        (Value::Str(a), Value::Str(b)) => Value::Str(format!("{}{}", a, b)),
                        (Value::Int(a), Value::Str(b)) => Value::Str(format!("{}{}", a, b)),
                        (Value::Str(a), Value::Int(b)) => Value::Str(format!("{}{}", a, b)),
                        _ => return Err("加法操作类型不匹配".to_string()),
                    },
                    BinOp::Sub => {
                        let right_val = self.evaluate(right)?;
                        int_op(left_val, right_val, |a, b| a - b)?
                    }
                    BinOp::Mul => {
                        let right_val = self.evaluate(right)?;
                        int_op(left_val, right_val, |a, b| a * b)?
                    }
                    BinOp::Div => {
                        let right_val = self.evaluate(right)?;
                        match (left_val, right_val) {
                            (Value::Int(_), Value::Int(0)) => {
                                return Err("除法除数不能为 0".to_string())
                            }
                            (Value::Int(a), Value::Int(b)) => Value::Int(a / b),
                            _ => return Err("算术操作需要整数".to_string()),
                        }
                    }
                    BinOp::Mod => {
                        let right_val = self.evaluate(right)?;
                        match (left_val, right_val) {
                            (Value::Int(_), Value::Int(0)) => {
                                return Err("取模除数不能为 0".to_string())
                            }
                            (Value::Int(a), Value::Int(b)) => Value::Int(a % b),
                            _ => return Err("算术操作需要整数".to_string()),
                        }
                    }
                    BinOp::Lt => {
                        let right_val = self.evaluate(right)?;
                        cmp_op(left_val, right_val, |a, b| a < b)?
                    }
                    BinOp::Gt => {
                        let right_val = self.evaluate(right)?;
                        cmp_op(left_val, right_val, |a, b| a > b)?
                    }
                    BinOp::Le => {
                        let right_val = self.evaluate(right)?;
                        cmp_op(left_val, right_val, |a, b| a <= b)?
                    }
                    BinOp::Ge => {
                        let right_val = self.evaluate(right)?;
                        cmp_op(left_val, right_val, |a, b| a >= b)?
                    }
                    BinOp::EqEq | BinOp::EqEqEq => {
                        Value::Bool(left_val == self.evaluate(right)?)
                    }
                    BinOp::Neq | BinOp::NeqEq => {
                        Value::Bool(left_val != self.evaluate(right)?)
                    }
                };

                Ok(result)
            }
            Expr::Unary(op, expr) => {
                let val = self.evaluate(expr)?;
                match op {
                    UnaryOp::Minus => match val {
                        Value::Int(n) => Ok(Value::Int(-n)),
                        _ => Err("取负操作要求整数".to_string()),
                    },
                    UnaryOp::Plus => match val {
                        Value::Int(n) => Ok(Value::Int(n)),
                        _ => Err("一元加号要求整数".to_string()),
                    },
                    UnaryOp::Not => Ok(Value::Bool(!is_truthy(&val))),
                }
            }
            Expr::Index(left, index) => {
                let container = self.evaluate(left)?;
                let index = self.evaluate(index)?;
                let idx = match index {
                    Value::Int(n) if n >= 0 => n as usize,
                    _ => return Err("索引必须是非负整数".to_string()),
                };

                match container {
                    Value::Array(items) => {
                        if idx >= items.len() {
                            Ok(Value::Undefined)
                        } else {
                            Ok(items[idx].clone())
                        }
                    }
                    _ => Err("只能对数组进行索引访问".to_string()),
                }
            }
            Expr::Call(callee, args) => {
                let callee = self.evaluate(callee)?;
                let evaluated_args: Vec<Value> = args
                    .iter()
                    .map(|a| self.evaluate(a))
                    .collect::<Result<_, _>>()?;

                match callee {
                    Value::Function(params, body) => {
                        if params.len() != evaluated_args.len() {
                            return Err(format!(
                                "函数 '{}' 期望 {} 个参数，但传入了 {} 个",
                                "<anonymous>",
                                params.len(),
                                evaluated_args.len()
                            ));
                        }

                        let parent_env = std::mem::replace(&mut self.env, Environment::new());
                        let mut func_env = Environment::new_enclosed(parent_env);
                        for (param, arg) in params.iter().zip(evaluated_args.into_iter()) {
                            func_env.define(param, arg);
                        }

                        match self.execute_block(&body, func_env)? {
                            ControlFlow::Return(v) => Ok(v),
                            ControlFlow::Value(_) => Ok(Value::Null),
                            _ => Err("break/continue 不能在函数体外使用".to_string()),
                        }
                    }
                    Value::NativeFunction(f) => f(&evaluated_args),
                    _ => Err("不是一个可调用对象".to_string()),
                }
            }
        }
    }
}

/// 整数算术运算辅助函数。
fn int_op(a: Value, b: Value, op: fn(i64, i64) -> i64) -> Result<Value, String> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(op(a, b))),
        _ => Err("算术操作需要整数".to_string()),
    }
}

/// 整数比较运算辅助函数。
fn cmp_op(a: Value, b: Value, op: fn(i64, i64) -> bool) -> Result<Value, String> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(op(a, b))),
        _ => Err("比较操作需要整数".to_string()),
    }
}

/// 将运行时值转为布尔真值。
fn is_truthy(val: &Value) -> bool {
    match val {
        Value::Bool(false) | Value::Null | Value::Undefined => false,
        Value::Int(0) => false,
        Value::Str(s) if s.is_empty() => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Program;
    use crate::parser::Parser;
    use crate::value::Value;

    fn parse(source: &str) -> Program {
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
        assert_eq!(interpreter.env.get("x").unwrap(), Value::Int(2));
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
        assert_eq!(interpreter.env.get("x").unwrap(), Value::Int(7));
    }

    #[test]
    fn function_expression_call() {
        let interpreter =
            run("ika add = fn(a, b) { return a + b; }; ika x = add(1, 2);");
        assert_eq!(interpreter.env.get("x").unwrap(), Value::Int(3));
    }

    #[test]
    fn array_access() {
        let interpreter = run("ika a = [1, 2, 3]; ika b = a[1];");
        assert_eq!(interpreter.env.get("b").unwrap(), Value::Int(2));
    }

    #[test]
    fn null_and_undefined_runtime_values() {
        let interpreter = run("ika a = null; ika b = undefined;");
        assert_eq!(interpreter.env.get("a").unwrap(), Value::Null);
        assert_eq!(interpreter.env.get("b").unwrap(), Value::Undefined);
    }

    #[test]
    fn short_circuit_and_skips_rhs() {
        let interpreter = run("ika x = 0; ika ok = false and (x = 1);");
        assert_eq!(interpreter.env.get("ok").unwrap(), Value::Bool(false));
        assert_eq!(interpreter.env.get("x").unwrap(), Value::Int(0));
    }

    #[test]
    fn short_circuit_or_skips_rhs() {
        let interpreter = run("ika x = 0; ika ok = true or (x = 1);");
        assert_eq!(interpreter.env.get("ok").unwrap(), Value::Bool(true));
        assert_eq!(interpreter.env.get("x").unwrap(), Value::Int(0));
    }

    #[test]
    fn while_break_and_continue_flow() {
        let interpreter = run(
            "ika x = 0;
            while x < 5 {
              x = x + 1;
              if x == 2 { continue; }
              if x == 4 { break; }
            }",
        );
        assert_eq!(interpreter.env.get("x").unwrap(), Value::Int(4));
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
        assert!(
            err == "break 不能在循环外使用" || err == "break/continue 不能在函数体外使用"
        );
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
    fn modulo_supports_integers() {
        let interpreter = run("ika x = 7 % 3;");
        assert_eq!(interpreter.env.get("x").unwrap(), Value::Int(1));
    }

    #[test]
    fn array_out_of_range_is_undefined() {
        let interpreter = run("ika a = [1, 2]; ika b = a[10];");
        assert_eq!(interpreter.env.get("b").unwrap(), Value::Undefined);
    }
}
