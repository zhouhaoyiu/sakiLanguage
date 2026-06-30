use crate::ast::*;
use crate::environment::Environment;
use crate::value::Value;
use std::collections::BTreeMap;

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

    /// 读取当前环境中的变量，供集成测试和外部调用检查结果。
    pub fn get(&self, name: &str) -> Result<Value, String> {
        self.env.get(name)
    }

    /// 执行整个程序。
    pub fn interpret(&mut self, program: &Program) -> Result<(), String> {
        for stmt in &program.statements {
            match self.execute(stmt)? {
                ControlFlow::Value(_) => {}
                ControlFlow::Return(_) => return Err("return 不能在函数外使用".to_string()),
                ControlFlow::Break | ControlFlow::Continue => {
                    return Err("break/continue 不能在循环外使用".to_string());
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
                match kind {
                    VarKind::Var => self.env.define_var(name, value),
                    VarKind::Let => self.env.define(name, value),
                    VarKind::Const => self.env.define_with(name, value, false),
                }
                Ok(ControlFlow::Value(Value::Null))
            }
            Stmt::FnDecl(name, params, body) => {
                let func = Value::Function(params.clone(), body.clone(), self.env.clone());
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
                    self.execute_block(then_stmts, Environment::new_enclosed(self.env.clone()))
                } else if let Some(stmts) = else_stmts {
                    self.execute_block(stmts, Environment::new_enclosed(self.env.clone()))
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

                        match self
                            .execute_block(body, Environment::new_enclosed(self.env.clone()))?
                        {
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
            Stmt::For(init, condition, update, body) => {
                self.execute_for(init.as_deref(), condition.as_ref(), update.as_ref(), body)
            }
            Stmt::Block(stmts) => {
                self.execute_block(stmts, Environment::new_enclosed(self.env.clone()))
            }
        }
    }

    /// 执行 for 循环，并把 let/const 初始化变量限制在循环作用域内。
    fn execute_for(
        &mut self,
        init: Option<&Stmt>,
        condition: Option<&Expr>,
        update: Option<&Expr>,
        body: &[Stmt],
    ) -> Result<ControlFlow, String> {
        let old_env = self.env.clone();
        let previous_depth = self.loop_depth;
        self.env = Environment::new_enclosed(old_env.clone());
        self.loop_depth += 1;

        let result = (|| -> Result<ControlFlow, String> {
            if let Some(stmt) = init {
                match self.execute(stmt)? {
                    ControlFlow::Value(_) => {}
                    ControlFlow::Return(v) => return Ok(ControlFlow::Return(v)),
                    ControlFlow::Break | ControlFlow::Continue => {
                        return Err("break/continue 不能在循环外使用".to_string());
                    }
                }
            }

            loop {
                if let Some(expr) = condition {
                    let value = self.evaluate(expr)?;
                    if !is_truthy(&value) {
                        break Ok(ControlFlow::Value(Value::Null));
                    }
                }

                match self.execute_block(body, Environment::new_enclosed(self.env.clone()))? {
                    ControlFlow::Value(_) | ControlFlow::Continue => {}
                    ControlFlow::Return(v) => break Ok(ControlFlow::Return(v)),
                    ControlFlow::Break => break Ok(ControlFlow::Value(Value::Null)),
                }

                if let Some(expr) = update {
                    self.evaluate(expr)?;
                }
            }
        })();

        self.loop_depth = previous_depth;
        self.env = old_env;
        result
    }

    /// 执行语句块并保持作用域隔离。
    fn execute_block(
        &mut self,
        stmts: &[Stmt],
        new_env: Environment,
    ) -> Result<ControlFlow, String> {
        let old_env = self.env.clone();
        self.env = new_env;
        let mut result = Value::Null;

        for stmt in stmts {
            match self.execute(stmt) {
                Err(err) => {
                    self.env = old_env;
                    return Err(err);
                }
                Ok(ControlFlow::Value(val)) => result = val,
                Ok(control) => {
                    self.env = old_env;
                    return Ok(control);
                }
            }
        }

        self.env = old_env;
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
            Expr::Object(entries) => {
                let mut values = BTreeMap::new();
                for (key, expr) in entries {
                    values.insert(key.clone(), self.evaluate(expr)?);
                }
                Ok(Value::Object(values))
            }
            Expr::Ident(name) => self.env.get(name),
            Expr::Assign(name, value) => {
                let value = self.evaluate(value)?;
                self.env.set(name, value.clone())?;
                Ok(value)
            }
            Expr::FunctionExpr(params, body) => Ok(Value::Function(
                params.clone(),
                body.clone(),
                self.env.clone(),
            )),
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
                                return Err("除法除数不能为 0".to_string());
                            }
                            (Value::Int(a), Value::Int(b)) => Value::Int(a / b),
                            _ => return Err("算术操作需要整数".to_string()),
                        }
                    }
                    BinOp::Mod => {
                        let right_val = self.evaluate(right)?;
                        match (left_val, right_val) {
                            (Value::Int(_), Value::Int(0)) => {
                                return Err("取模除数不能为 0".to_string());
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
                    BinOp::EqEq | BinOp::EqEqEq => Value::Bool(left_val == self.evaluate(right)?),
                    BinOp::Neq | BinOp::NeqEq => Value::Bool(left_val != self.evaluate(right)?),
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
            Expr::Property(object, name) => match self.evaluate(object)? {
                Value::Object(entries) => {
                    Ok(entries.get(name).cloned().unwrap_or(Value::Undefined))
                }
                _ => Err("只能对对象进行属性访问".to_string()),
            },
            Expr::Call(callee, args) => {
                let callee = self.evaluate(callee)?;
                let evaluated_args: Vec<Value> = args
                    .iter()
                    .map(|a| self.evaluate(a))
                    .collect::<Result<_, _>>()?;

                match callee {
                    Value::Function(params, body, closure_env) => {
                        if params.len() != evaluated_args.len() {
                            return Err(format!(
                                "函数 '{}' 期望 {} 个参数，但传入了 {} 个",
                                "<anonymous>",
                                params.len(),
                                evaluated_args.len()
                            ));
                        }

                        let func_env = Environment::new_function(closure_env);
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
