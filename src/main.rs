use saki_lang::interpreter::Interpreter;
use saki_lang::parser::Parser;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;

/// 程序入口：支持文件执行与 REPL 模式。
fn main() {
    // 读取命令行参数。
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        // 文件执行模式
        // 解析脚本文件名。
        let filename = &args[1];
        match fs::read_to_string(filename) {
            Ok(source) => {
                // 构建解析器。
                let mut parser = Parser::new(&source);
                match parser.parse_program() {
                    Ok(program) => {
                        // 创建解释器。
                        let mut interpreter = Interpreter::new();
                        if let Err(e) = interpreter.interpret(&program) {
                            eprintln!("运行时错误: {}", e);
                            process::exit(1);
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
        // 保持同一个解释器以保留环境。
        let mut interpreter = Interpreter::new();
        loop {
            print!(">>> ");
            io::stdout().flush().unwrap();
            // 读取一行输入。
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                break;
            }
            // 清理首尾空白。
            let trimmed = input.trim();
            if trimmed == "exit" {
                break;
            }
            if trimmed.is_empty() {
                continue;
            }
            // 基于当前输入构建解析器。
            let mut parser = Parser::new(trimmed);
            match parser.parse_program() {
                Ok(program) => {
                    if let Err(e) = interpreter.interpret(&program) {
                        eprintln!("运行时错误: {}", e);
                    }
                }
                Err(e) => eprintln!("解析错误: {}", e),
            }
        }
    }
}
