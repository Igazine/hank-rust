use crate::types::{Value, Scope, Expr, Arc, Resource};
use crate::lexer::{Lexer, Token};
use crate::parser::Parser;
use crate::interpreter::{Interpreter, EvalResult, HankScope};
use std::collections::HashMap;
use std::cell::RefCell;

/**
 * A Hank Host Runner.
 * Handles resource orchestration, macro resolution, and AST caching.
 * Platform-agnostic: uses the Resource model for all content retrieval.
 */
pub struct Runner {
    resource_cache: RefCell<HashMap<String, Arc<dyn Resource>>>,
    pub core_scope: Arc<dyn Scope>,
}

impl Runner {
    pub fn new() -> Self {
        Self {
            resource_cache: RefCell::new(HashMap::new()),
            core_scope: Arc::new(HankScope::new()),
        }
    }

    pub fn register_module(&self, name: &str, tasks: HashMap<String, Value>) {
        self.core_scope.set(name, Value::Object(Arc::new(RefCell::new(tasks))));
    }

    /**
     * Pre-loads and caches a resource for execution.
     */
    pub fn load(&self, resource: Arc<dyn Resource>, stack: Arc<RefCell<Vec<String>>>) -> Result<Expr, String> {
        // Check cache
        if let Some(cached) = self.resource_cache.borrow().get(resource.id()) {
            if let Some(ast) = cached.ast() {
                return Ok(ast);
            }
        }

        // Circular Dependency Check
        if stack.borrow().contains(&resource.id().to_string()) {
            return Err(format!("Circular Dependency: {}", resource.id()));
        }

        // Reconcile with cache
        let active_resource = {
            let mut cache = self.resource_cache.borrow_mut();
            if !cache.contains_key(resource.id()) {
                cache.insert(resource.id().to_string(), resource.clone());
                resource
            } else {
                cache.get(resource.id()).unwrap().clone()
            }
        };

        active_resource.load()?;
        let content = active_resource.content().ok_or_else(|| format!("Resource content not loaded: {}", active_resource.id()))?;

        stack.borrow_mut().push(active_resource.id().to_string());

        let mut lexer = Lexer::new(&content);
        let tokens = lexer.tokenize();
        
        let active_resource_inner = active_resource.clone();
        // This is the tricky part in Rust. We need a way to call load recursively.
        // We can use a reference to self if Parser doesn't require 'static.
        // But Parser stores the closure.
        // Let's use a trick: we'll use a Weak reference to the Runner or similar if needed,
        // or just accept that we need to pass a context.
        // Actually, we can use a "recursive resolver" closure that captures a reference to self.
        
        let runner_ptr: *const Runner = self;
        let stack_inner = stack.clone();

        let mut parser = Parser::new(tokens, active_resource.id().to_string(), Box::new(move |macro_path| {
            let m_res = active_resource_inner.resolve(&macro_path)?;
            // SAFETY: We know the Runner exists because we are running inside its run/load method.
            unsafe {
                (*runner_ptr).load(m_res.into(), stack_inner.clone())
            }
        }));

        let ast = parser.parse().map_err(|e| e.to_string())?;
        active_resource.set_ast(ast.clone());
        
        stack.borrow_mut().pop();
        Ok(ast)
    }

    pub fn unload(&self, resource: &dyn Resource) {
        self.resource_cache.borrow_mut().remove(resource.id());
    }

    pub fn run(&self, resource: Arc<dyn Resource>, args: Vec<Value>) -> Result<Value, String> {
        let stack = Arc::new(RefCell::new(vec![]));
        let ast = self.load(resource, stack)?;

        let mut interp = Interpreter::new(None, self.core_scope.clone());
        let script_task = match interp.run(&ast) {
            Value::Task(t) => t,
            _ => return Err("Script did not evaluate to a Task definition.".into()),
        };

        match interp.call(&Value::Task(script_task), args, &interp.global_scope) {
            EvalResult::Value(v) | EvalResult::Return(v) => Ok(v),
            EvalResult::Error(e) => Err(e),
        }
    }
}
