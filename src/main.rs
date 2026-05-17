mod token;
mod lexer;
mod ast;
mod parser;
mod value;
mod environment;
mod interpreter;

use interpreter::Interpreter;
use parser::Parser;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        // 文件执行模式
        let filename = &args[1];
        match fs::read_to_string(filename) {
            Ok(source) => {
                let mut parser = Parser::new(&source);
                match parser.parse_program() {
                    Ok(program) => {
                        let mut interpreter = Interpreter::new();
                        if let Err(e) = interpreter.interpret(&program) {
                            if !e.starts_with("__return__") {
                                eprintln!("运行时错误: {}", e);
                                process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("解析错误: {}", e);
                        process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("无法读取文件 '{}': {}", filename, e);
                process::exit(1);
            }
        }
    } else {
        // REPL 模式
        println!("欢迎使用 Saki 语言 REPL. 输入 'exit' 退出.");
        let mut interpreter = Interpreter::new();
        loop {
            print!(">>> ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                break;
            }
            let trimmed = input.trim();
            if trimmed == "exit" {
                break;
            }
            if trimmed.is_empty() {
                continue;
            }
            let mut parser = Parser::new(trimmed);
            match parser.parse_program() {
                Ok(program) => {
                    if let Err(e) = interpreter.interpret(&program) {
                        if !e.starts_with("__return__") {
                            eprintln!("运行时错误: {}", e);
                        }
                    }
                }
                Err(e) => eprintln!("解析错误: {}", e),
            }
        }
    }
}