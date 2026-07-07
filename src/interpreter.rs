use crate::ast::*;
use crate::environment::Environment;
use crate::value::{ObjectShape, ObjectValue, Value};
use std::collections::HashMap;
use std::rc::Rc;

const PROPERTY_TIER_THRESHOLD: usize = 2;

/// 语法树解释执行器。
pub struct Interpreter {
    /// 当前执行环境。
    env: Environment,
    /// 当前循环层数。
    loop_depth: usize,
    /// 对象形状缓存。
    shapes: HashMap<Vec<String>, Rc<ObjectShape>>,
    /// 属性访问点反馈。
    property_feedback: HashMap<usize, PropertyFeedback>,
    /// 字节码执行次数。
    bytecode_runs: usize,
    /// 树遍历执行次数。
    tree_walk_runs: usize,
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

#[derive(Debug, Clone, Copy, PartialEq)]
enum PropertyTier {
    Generic,
    Monomorphic,
}

#[derive(Debug, Clone)]
struct PropertyFeedback {
    key: String,
    shape_id: usize,
    slot: usize,
    hits: usize,
    misses: usize,
    tier: PropertyTier,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// 解释器优化状态。
pub struct OptimizationStats {
    /// 走字节码路径的次数。
    pub bytecode_runs: usize,
    /// 走树遍历路径的次数。
    pub tree_walk_runs: usize,
    /// 属性访问反馈点数量。
    pub property_feedback_sites: usize,
    /// 已升为单态 inline cache 的属性访问点数量。
    pub monomorphic_property_sites: usize,
    /// 属性 fast path 命中次数。
    pub property_cache_hits: usize,
}

impl Interpreter {
    /// 创建解释器并初始化全局环境。
    pub fn new() -> Self {
        Interpreter {
            env: Environment::new(),
            loop_depth: 0,
            shapes: HashMap::new(),
            property_feedback: HashMap::new(),
            bytecode_runs: 0,
            tree_walk_runs: 0,
        }
    }

    /// 读取当前环境中的变量，供集成测试和外部调用检查结果。
    pub fn get(&self, name: &str) -> Result<Value, String> {
        self.env.get(name)
    }

    /// 返回优化反馈统计。
    pub fn optimization_stats(&self) -> OptimizationStats {
        OptimizationStats {
            bytecode_runs: self.bytecode_runs,
            tree_walk_runs: self.tree_walk_runs,
            property_feedback_sites: self.property_feedback.len(),
            monomorphic_property_sites: self
                .property_feedback
                .values()
                .filter(|feedback| feedback.tier == PropertyTier::Monomorphic)
                .count(),
            property_cache_hits: self.property_feedback.values().map(|f| f.hits).sum(),
        }
    }

    /// 执行整个程序。
    pub fn interpret(&mut self, program: &Program) -> Result<(), String> {
        self.tree_walk_runs += 1;
        self.interpret_tree(program)
    }

    /// 显式使用字节码执行可编译程序；不能编译时回退树遍历。
    pub fn interpret_bytecode(&mut self, program: &Program) -> Result<(), String> {
        if let Some(bytecode) = crate::bytecode::compile_program(program) {
            self.bytecode_runs += 1;
            return self.execute_bytecode(&bytecode);
        }

        self.tree_walk_runs += 1;
        self.interpret_tree(program)
    }

    fn interpret_tree(&mut self, program: &Program) -> Result<(), String> {
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

    /// 执行可编译的直线字节码。
    fn execute_bytecode(&mut self, code: &[crate::bytecode::Instr]) -> Result<(), String> {
        let mut stack = Vec::new();

        for instr in code {
            match instr {
                crate::bytecode::Instr::LoadInt(n) => stack.push(Value::Int(*n)),
                crate::bytecode::Instr::LoadStr(s) => stack.push(Value::Str(s.clone())),
                crate::bytecode::Instr::LoadBool(b) => stack.push(Value::Bool(*b)),
                crate::bytecode::Instr::LoadNull => stack.push(Value::Null),
                crate::bytecode::Instr::LoadUndefined => stack.push(Value::Undefined),
                crate::bytecode::Instr::LoadVar(name) => stack.push(self.env.get(name)?),
                crate::bytecode::Instr::Define(name, kind) => {
                    let value = stack.pop().ok_or_else(|| "字节码栈下溢".to_string())?;
                    match kind {
                        VarKind::Var => self.env.define_var(name, value),
                        VarKind::Let => self.env.define(name, value),
                        VarKind::Const => self.env.define_with(name, value, false),
                    }
                }
                crate::bytecode::Instr::Store(name) => {
                    let value = stack.pop().ok_or_else(|| "字节码栈下溢".to_string())?;
                    self.env.set(name, value.clone())?;
                    stack.push(value);
                }
                crate::bytecode::Instr::Unary(op) => {
                    let value = stack.pop().ok_or_else(|| "字节码栈下溢".to_string())?;
                    stack.push(self.apply_unary(op, value)?);
                }
                crate::bytecode::Instr::Binary(op) => {
                    let right = stack.pop().ok_or_else(|| "字节码栈下溢".to_string())?;
                    let left = stack.pop().ok_or_else(|| "字节码栈下溢".to_string())?;
                    stack.push(self.apply_binary(left, op, right)?);
                }
                crate::bytecode::Instr::MakeObject(keys) => {
                    if stack.len() < keys.len() {
                        return Err("字节码栈下溢".to_string());
                    }
                    let slots = stack.split_off(stack.len() - keys.len());
                    let shape = self.shape_for(keys.clone());
                    stack.push(Value::Object(ObjectValue::new(shape, slots)));
                }
                crate::bytecode::Instr::GetProperty { site, name } => {
                    let value = stack.pop().ok_or_else(|| "字节码栈下溢".to_string())?;
                    match value {
                        Value::Object(object) => {
                            stack.push(self.read_property(*site, object, name));
                        }
                        _ => return Err("只能对对象进行属性访问".to_string()),
                    }
                }
                crate::bytecode::Instr::Pop => {
                    stack.pop();
                }
            }
        }

        Ok(())
    }

    /// 获取或创建对象形状。
    fn shape_for(&mut self, keys: Vec<String>) -> Rc<ObjectShape> {
        if let Some(shape) = self.shapes.get(&keys) {
            return shape.clone();
        }

        let shape = Rc::new(ObjectShape::new(keys.clone()));
        self.shapes.insert(keys, shape.clone());
        shape
    }

    /// 读取对象属性，带单态 inline cache。
    fn read_property(&mut self, site: usize, object: ObjectValue, name: &str) -> Value {
        let shape_id = object.shape_id();
        if let Some(feedback) = self.property_feedback.get_mut(&site) {
            if feedback.tier == PropertyTier::Monomorphic
                && feedback.shape_id == shape_id
                && feedback.key == name
            {
                feedback.hits += 1;
                return object
                    .get_slot(feedback.slot)
                    .cloned()
                    .unwrap_or(Value::Undefined);
            }
        }

        let Some(slot) = object.slot(name) else {
            return Value::Undefined;
        };
        let value = object.get_slot(slot).cloned().unwrap_or(Value::Undefined);
        let feedback = self
            .property_feedback
            .entry(site)
            .or_insert_with(|| PropertyFeedback {
                key: name.to_string(),
                shape_id,
                slot,
                hits: 0,
                misses: 0,
                tier: PropertyTier::Generic,
            });

        if feedback.key == name && feedback.shape_id == shape_id && feedback.slot == slot {
            feedback.hits += 1;
            if feedback.hits >= PROPERTY_TIER_THRESHOLD {
                feedback.tier = PropertyTier::Monomorphic;
            }
        } else {
            feedback.key = name.to_string();
            feedback.shape_id = shape_id;
            feedback.slot = slot;
            feedback.hits = 1;
            feedback.misses += 1;
            feedback.tier = PropertyTier::Generic;
        }

        value
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

    fn apply_unary(&self, op: &UnaryOp, value: Value) -> Result<Value, String> {
        match op {
            UnaryOp::Minus => match value {
                Value::Int(n) => Ok(Value::Int(-n)),
                _ => Err("取负操作要求整数".to_string()),
            },
            UnaryOp::Plus => match value {
                Value::Int(n) => Ok(Value::Int(n)),
                _ => Err("一元加号要求整数".to_string()),
            },
            UnaryOp::Not => Ok(Value::Bool(!is_truthy(&value))),
        }
    }

    fn apply_binary(&self, left: Value, op: &BinOp, right: Value) -> Result<Value, String> {
        match op {
            BinOp::And | BinOp::Or => Err("字节码不支持短路逻辑".to_string()),
            BinOp::Add => match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                (Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{}{}", a, b))),
                (Value::Int(a), Value::Str(b)) => Ok(Value::Str(format!("{}{}", a, b))),
                (Value::Str(a), Value::Int(b)) => Ok(Value::Str(format!("{}{}", a, b))),
                _ => Err("加法操作类型不匹配".to_string()),
            },
            BinOp::Sub => int_op(left, right, |a, b| a - b),
            BinOp::Mul => int_op(left, right, |a, b| a * b),
            BinOp::Div => match (left, right) {
                (Value::Int(_), Value::Int(0)) => Err("除法除数不能为 0".to_string()),
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a / b)),
                _ => Err("算术操作需要整数".to_string()),
            },
            BinOp::Mod => match (left, right) {
                (Value::Int(_), Value::Int(0)) => Err("取模除数不能为 0".to_string()),
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),
                _ => Err("算术操作需要整数".to_string()),
            },
            BinOp::Lt => cmp_op(left, right, |a, b| a < b),
            BinOp::Gt => cmp_op(left, right, |a, b| a > b),
            BinOp::Le => cmp_op(left, right, |a, b| a <= b),
            BinOp::Ge => cmp_op(left, right, |a, b| a >= b),
            BinOp::EqEq | BinOp::EqEqEq => Ok(Value::Bool(left == right)),
            BinOp::Neq | BinOp::NeqEq => Ok(Value::Bool(left != right)),
        }
    }

    /// 计算表达式并返回运行时值。
    fn evaluate(&mut self, expr: &Expr) -> Result<Value, String> {
        let expr_site = expr as *const Expr as usize;
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
                let mut keys = Vec::with_capacity(entries.len());
                let mut slots = Vec::with_capacity(entries.len());
                for (key, expr) in entries {
                    keys.push(key.clone());
                    slots.push(self.evaluate(expr)?);
                }
                let shape = self.shape_for(keys);
                Ok(Value::Object(ObjectValue::new(shape, slots)))
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
                self.apply_unary(op, val)
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
                Value::Object(object) => Ok(self.read_property(expr_site, object, name)),
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
