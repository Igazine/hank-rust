use crate::types::{Value, Scope, Expr, Arc, Resource, HankError, HankErrorValue, TaskValue, HankExtension, EvalResult};
use crate::lexer::{Lexer, Token};
use crate::parser::Parser;
use crate::interpreter::{Interpreter, HankScope};
use crate::error_registry::HankErrorRegistry;
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
    pub localization: RefCell<HashMap<i32, String>>,
}

impl Runner {
    pub fn new() -> Self {
        Self {
            resource_cache: RefCell::new(HashMap::new()),
            core_scope: Arc::new(HankScope::new(None)),
            localization: RefCell::new(HashMap::new()),
        }
    }

    /**
     * Registers a localization map (Code -> Template).
     */
    pub fn register_localization(&self, map: HashMap<i32, String>) {
        for (code, tmpl) in map {
            self.localization.borrow_mut().insert(code, tmpl);
        }
    }

    pub fn register_module(&self, name: &str, tasks: HashMap<String, crate::types::NativeFunc>) {
        let mut module_obj = HashMap::new();
        for (t_name, func) in tasks {
            module_obj.insert(t_name.clone(), Value::Task(Arc::new(crate::types::TaskValue::Native {
                name: format!("{}.{}", name, t_name),
                func,
            })));
        }
        self.core_scope.set(name, Value::Map(Arc::new(RefCell::new(module_obj))));
    }

    pub fn register_extension(&self, ext: Box<dyn crate::types::HankExtension>) {
        let mods = ext.get_modules();
        for (name, tasks) in mods {
            self.register_module(&name, tasks);
        }
    }

    /**
     * Pre-loads and caches a resource for execution.
     */
    pub fn load(&self, resource: Arc<dyn Resource>, stack: Arc<RefCell<Vec<String>>>) -> Result<Expr, HankErrorValue> {
        // Check cache
        if let Some(cached) = self.resource_cache.borrow().get(resource.id()) {
            if let Some(ast) = cached.ast() {
                return Ok(ast);
            }
        }

        // Circular Dependency Check
        if stack.borrow().contains(&resource.id().to_string()) {
            return Err(HankErrorRegistry::create(HankError::CircularDependency, vec![resource.id().to_string()], None, None, None));
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

        active_resource.load().map_err(|e| HankErrorValue { code: HankError::ResourceContentNotLoaded, message: e })?;
        let content = active_resource.content().ok_or_else(|| {
            HankErrorRegistry::create(HankError::ResourceContentNotLoaded, vec![active_resource.id().to_string()], None, None, None)
        })?;

        stack.borrow_mut().push(active_resource.id().to_string());

        let mut lexer = Lexer::new(&content);
        let tokens = lexer.tokenize();
        
        let active_resource_inner = active_resource.clone();
        let runner_ptr: *const Runner = self;
        let stack_inner = stack.clone();

        let mut parser = Parser::new(tokens, active_resource.id().to_string(), Box::new(move |macro_path| {
            let m_res = active_resource_inner.resolve(&macro_path).map_err(|e| {
                HankErrorRegistry::create(HankError::MacroResourceNotFound, vec![macro_path.clone()], None, None, None)
            })?;
            // SAFETY: We know the Runner exists because we are running inside its run/load method.
            unsafe {
                (*runner_ptr).load(m_res.into(), stack_inner.clone())
            }
        }));

        let ast = parser.parse()?;
        active_resource.set_ast(ast.clone());
        
        stack.borrow_mut().pop();
        Ok(ast)
    }

    pub fn unload(&self, resource: &dyn Resource) {
        self.resource_cache.borrow_mut().remove(resource.id());
    }

    pub fn run(&self, resource: Arc<dyn Resource>, args: Vec<Value>) -> Result<Value, HankErrorValue> {
        let stack = Arc::new(RefCell::new(vec![]));
        let ast = self.load(resource, stack)?;

        let mut interp = Interpreter::new(None, self.core_scope.clone(), self.localization.borrow().clone());
        let script_res = interp.run(&ast);

        if let Value::Task(script_task) = script_res {
            match interp.call(&Value::Task(script_task), args, &interp.global_scope) {
                EvalResult::Value(v) | EvalResult::Return(v) => Ok(v),
                EvalResult::Break => Ok(Value::Void),
                EvalResult::Error(v) => {
                    // Convert native error back to Host error if it bubbled up to the runner
                    if let Value::Error(e) = &v {
                        let mut msg = self.localization.borrow().get(&(e.code as i32)).cloned().unwrap_or_else(|| "Uncaught Hank Error".into());
                        // Simplistic formatting for runner-level exit
                        Ok(v)
                    } else {
                        Ok(v)
                    }
                }
            }
        } else if let Value::Error(_) = script_res {
            Ok(script_res)
        } else {
            Err(HankErrorRegistry::create(HankError::ScriptMustBeTask, vec![], None, None, None))
        }
    }
}
