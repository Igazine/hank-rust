use crate::types::{Value, Scope, Expr, Arc};
use crate::lexer::{Lexer, Token};
use crate::parser::Parser;
use crate::interpreter::{Interpreter, EvalResult, HALScope};
use std::collections::HashMap;
use std::cell::RefCell;

#[cfg(not(target_arch = "wasm32"))]
pub type ReadFileFn = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;
#[cfg(target_arch = "wasm32")]
pub type ReadFileFn = Arc<dyn Fn(&str) -> Result<String, String>>;

#[cfg(not(target_arch = "wasm32"))]
pub type ResolvePathFn = Arc<dyn Fn(&str, &str) -> Result<String, String> + Send + Sync>;
#[cfg(target_arch = "wasm32")]
pub type ResolvePathFn = Arc<dyn Fn(&str, &str) -> Result<String, String>>;

pub struct Runner {
    path_cache: HashMap<String, String>,
    ast_cache: HashMap<String, Expr>,
    macro_map: HashMap<String, String>,
    pub core_scope: Arc<dyn Scope>,
    read_file: ReadFileFn,
    resolve_path: ResolvePathFn,
}

impl Runner {
    pub fn new(read_file: ReadFileFn, resolve_path: ResolvePathFn) -> Self {
        Self {
            path_cache: HashMap::new(),
            ast_cache: HashMap::new(),
            macro_map: HashMap::new(),
            core_scope: Arc::new(HALScope::new()),
            read_file,
            resolve_path,
        }
    }

    pub fn register_module(&self, name: &str, tasks: HashMap<String, Value>) {
        self.core_scope.set(name, Value::Object(Arc::new(RefCell::new(tasks))));
    }

    pub fn load(&mut self, script_path: &str) -> Result<String, String> {
        let abs_path = (self.resolve_path)(script_path, "")?;
        if self.ast_cache.contains_key(&abs_path) { return Ok(abs_path); }

        self.preprocess(&abs_path, &mut vec![])?;

        let content = self.path_cache.get(&abs_path).ok_or_else(|| format!("File not loaded: {}", abs_path))?;
        let mut lexer = Lexer::new(content);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, abs_path.clone(), self.macro_map.clone());
        let ast = parser.parse().map_err(|e| e.to_string())?;

        self.ast_cache.insert(abs_path.clone(), ast);
        Ok(abs_path)
    }

    pub fn unload(&mut self, script_path: &str) {
        if let Ok(abs_path) = (self.resolve_path)(script_path, "") {
            self.ast_cache.remove(&abs_path);
            self.path_cache.remove(&abs_path);
        }
    }

    pub fn run(&mut self, script_path: &str, args: Vec<Value>) -> Result<Value, String> {
        let abs_path = self.load(script_path)?;
        let ast = self.ast_cache.get(&abs_path).unwrap();

        let mut interp = Interpreter::new(None, self.core_scope.clone());
        let script_task = match interp.run(ast) {
            Value::Task(t) => t,
            _ => return Err("Script did not evaluate to a Task definition.".into()),
        };

        match interp.call(&Value::Task(script_task), args, &interp.global_scope) {
            EvalResult::Value(v) | EvalResult::Return(v) => Ok(v),
            EvalResult::Error(e) => Err(e),
        }
    }

    fn preprocess(&mut self, path: &str, stack: &mut Vec<String>) -> Result<(), String> {
        if stack.contains(&path.to_string()) { return Err(format!("Circular Dependency: {}", path)); }
        if self.path_cache.contains_key(path) { return Ok(()); }

        let content = (self.read_file)(path)?;
        self.path_cache.insert(path.to_string(), content.clone());
        
        stack.push(path.to_string());
        let macros = self.scan_macros(&content);
        for m in macros {
            let m_path = (self.resolve_path)(&m, path)?;
            self.preprocess(&m_path, stack)?;
            self.macro_map.insert(m, self.path_cache.get(&m_path).unwrap().clone());
        }
        stack.pop();
        Ok(())
    }

    fn scan_macros(&self, content: &str) -> Vec<String> {
        let mut lexer = Lexer::new(content);
        let tokens = lexer.tokenize();
        let mut macros = vec![];
        for i in 0..tokens.len().saturating_sub(1) {
            if matches!(tokens[i].0, Token::At) {
                let next = &tokens[i+1].0;
                match next {
                    Token::String(s) | Token::Identifier(s) => {
                        macros.push(s.clone());
                    },
                    _ => {}
                }
            }
        }
        macros
    }
}
