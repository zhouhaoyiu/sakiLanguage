use std::collections::HashMap;
use crate::value::Value;

#[derive(Clone)]
pub struct Environment {
    variables: HashMap<String, Value>,
    parent: Option<Box<Environment>>,
}

impl Environment {
    pub fn new() -> Self {
        let mut env = Environment {
            variables: HashMap::new(),
            parent: None,
        };
        // 注册内建函数 saki
        env.variables
            .insert("saki".to_string(), Value::NativeFunction(native_saki));
        env
    }

    pub fn new_enclosed(parent: Environment) -> Self {
        Environment {
            variables: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }

    pub fn get(&self, name: &str) -> Result<Value, String> {
        if let Some(val) = self.variables.get(name) {
            Ok(val.clone())
        } else if let Some(parent) = &self.parent {
            parent.get(name)
        } else {
            Err(format!("未定义的变量 '{}'", name))
        }
    }

    // 暂时移除未使用的 set 方法
    // 未来需要变量赋值时可以加上：
    // pub fn set(&mut self, name: &str, value: Value) { ... }

    pub fn define(&mut self, name: &str, value: Value) {
        self.variables.insert(name.to_string(), value);
    }
}

fn native_saki(args: &[Value]) -> Result<Value, String> {
    for val in args {
        print!("{}", val);
    }
    println!();
    Ok(Value::Null)
}