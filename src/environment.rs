use std::collections::HashMap;
use crate::value::Value;

#[derive(Clone)]
/// 运行时变量作用域。
pub struct Environment {
    /// 当前作用域的变量表。
    variables: HashMap<String, Value>,
    /// 指向外层作用域。
    parent: Option<Box<Environment>>,
}

impl Environment {
    /// 创建顶层作用域并注册内建函数。
    pub fn new() -> Self {
        // 初始化环境对象。
        let mut env = Environment {
            variables: HashMap::new(),
            parent: None,
        };
        // 注册内建函数 saki
        env.variables
            .insert("saki".to_string(), Value::NativeFunction(native_saki));
        // 返回初始化后的环境。
        env
    }

    /// 创建带父作用域的新环境。
    pub fn new_enclosed(parent: Environment) -> Self {
        Environment {
            variables: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }

    /// 在作用域链中查找变量。
    pub fn get(&self, name: &str) -> Result<Value, String> {
        // 先查当前作用域。
        if let Some(val) = self.variables.get(name) {
            Ok(val.clone())
        } else if let Some(parent) = &self.parent {
            // 递归查找父作用域。
            parent.get(name)
        } else {
            // 未找到变量时返回错误。
            Err(format!("未定义的变量 '{}'", name))
        }
    }

    // 暂时移除未使用的 set 方法
    // 未来需要变量赋值时可以加上：
    // pub fn set(&mut self, name: &str, value: Value) { ... }

    /// 在当前作用域中定义变量。
    pub fn define(&mut self, name: &str, value: Value) {
        // 写入变量绑定。
        self.variables.insert(name.to_string(), value);
    }
}

/// 内建输出函数：打印参数并换行。
fn native_saki(args: &[Value]) -> Result<Value, String> {
    // 逐个输出参数。
    for val in args {
        print!("{}", val);
    }
    // 追加换行。
    println!();
    // 返回空值。
    Ok(Value::Null)
}