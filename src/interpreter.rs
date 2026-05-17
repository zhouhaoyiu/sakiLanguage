use crate::ast::*;
use crate::environment::Environment;
use crate::value::Value;

/// 用于在错误通道携带返回值的前缀。
const RETURN_PREFIX: &str = "__return__";

/// 语法树解释执行器。
pub struct Interpreter {
    /// 当前执行环境。
    env: Environment,
}

impl Interpreter {
    /// 创建解释器并初始化全局环境。
    pub fn new() -> Self {
        Interpreter {
            env: Environment::new(),
        }
    }

    /// 执行整个程序。
    pub fn interpret(&mut self, program: &Program) -> Result<(), String> {
        for stmt in &program.statements {
            self.execute(stmt)?;
        }
        Ok(())
    }

    /// 执行一条语句并返回结果值。
    fn execute(&mut self, stmt: &Stmt) -> Result<Value, String> {
        match stmt {
            Stmt::VarDecl(name, expr) => {
                // 计算变量初始值。
                let value = self.evaluate(expr)?;
                // 写入当前环境。
                self.env.define(name, value);
                Ok(Value::Null)
            }
            Stmt::FnDecl(name, params, body) => {
                // 构造函数值。
                let func = Value::Function(params.clone(), body.clone());
                // 注册函数名。
                self.env.define(name, func);
                Ok(Value::Null)
            }
            Stmt::ExprStmt(expr) => {
                self.evaluate(expr)?;
                Ok(Value::Null)
            }
            Stmt::Return(expr) => {
                // 解析返回值表达式。
                let value = match expr {
                    Some(e) => self.evaluate(e)?,
                    None => Value::Null,
                };
                // 使用错误通道向外层传播返回值。
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
        // 保存旧环境并切换到新作用域。
        let old_env = std::mem::replace(&mut self.env, new_env);
        // 记录块内最后的计算结果。
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

        // 恢复旧环境并返回结果。
        self.env = old_env;
        Ok(result)
    }

    /// 计算表达式并返回运行时值。
    fn evaluate(&mut self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::IntLit(n) => Ok(Value::Int(*n)),
            Expr::StrLit(s) => Ok(Value::Str(s.clone())),
            Expr::BoolLit(b) => Ok(Value::Bool(*b)),
            Expr::Ident(name) => self.env.get(name),
            Expr::Binary(left, op, right) => {
                // 先计算左右操作数。
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
                        // 逻辑与采用真值判断。
                        Ok(Value::Bool(is_truthy(&left_val) && is_truthy(&right_val)))
                    }
                    BinOp::Or => {
                        // 逻辑或采用真值判断。
                        Ok(Value::Bool(is_truthy(&left_val) || is_truthy(&right_val)))
                    }
                }
            }
            Expr::Unary(op, expr) => {
                // 先计算一元操作数。
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
                // 解析函数值。
                let func = self.env.get(name)?;
                // 计算所有实参。
                let evaluated_args: Vec<Value> = args
                    .iter()
                    .map(|a| self.evaluate(a))
                    .collect::<Result<_, _>>()?;

                match func {
                    Value::Function(params, body) => {
                        // 校验参数数量。
                        if params.len() != evaluated_args.len() {
                            return Err(format!(
                                "函数 '{}' 期望 {} 个参数，但传入了 {} 个",
                                name,
                                params.len(),
                                evaluated_args.len()
                            ));
                        }
                        // 创建函数调用的局部环境。
                        let mut func_env = Environment::new_enclosed(self.env.clone());
                        // 绑定形参与实参。
                        for (param, arg) in params.iter().zip(evaluated_args.into_iter()) {
                            func_env.define(param, arg);
                        }
                        match self.execute_block(&body, func_env) {
                            Ok(_) => Ok(Value::Null),
                            Err(e) => {
                                if e.starts_with(RETURN_PREFIX) {
                                    // 提取返回值字符串。
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
        Value::Bool(false) | Value::Null => false,
        Value::Int(0) => false,
        Value::Str(s) if s.is_empty() => false,
        _ => true,
    }
}

/// 从返回值字符串解析为运行时值。
fn parse_return_value(s: &str) -> Value {
    if s.starts_with("Int(") {
        // 提取整数内容。
        let num = s.trim_start_matches("Int(").trim_end_matches(')');
        Value::Int(num.parse().unwrap_or(0))
    } else if s.starts_with("Str(") {
        // 提取字符串内容。
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