use crate::types::{Expr, Value, TaskValue, ExecutionContext, Scope, Arc};
use std::collections::HashMap;
use std::cell::RefCell;

pub struct Interpreter {
    pub global_scope: Arc<dyn Scope>,
    pub core_scope: Arc<dyn Scope>,
    _depth: usize,
}

const MAX_DEPTH: usize = 500;

#[derive(Debug)]
pub struct HALScope {
    pub values: RefCell<HashMap<String, Value>>,
    pub parent: Option<Arc<dyn Scope>>,
}

impl HALScope {
    pub fn new() -> Self {
        Self {
            values: RefCell::new(HashMap::new()),
            parent: None,
        }
    }
}

impl Scope for HALScope {
    fn get(&self, name: &str) -> Value {
        if let Some(val) = self.values.borrow().get(name) { return val.clone(); }
        if let Some(parent) = &self.parent { return parent.get(name); }
        Value::Void
    }
    fn set(&self, name: &str, val: Value) { self.values.borrow_mut().insert(name.to_string(), val); }
    fn exists(&self, name: &str) -> bool {
        if self.values.borrow().contains_key(name) { return true; }
        if let Some(parent) = &self.parent { return parent.exists(name); }
        false
    }
}

pub enum EvalResult {
    Value(Value),
    Return(Value),
    Error(String),
}

impl Interpreter {
    pub fn new(parent_scope: Option<Arc<dyn Scope>>, core_scope: Arc<dyn Scope>) -> Self {
        let global = parent_scope.unwrap_or_else(|| Arc::new(HALScope {
            values: RefCell::new(HashMap::new()),
            parent: Some(core_scope.clone()),
        }));
        Self { global_scope: global, core_scope, _depth: 0 }
    }

    pub fn run(&mut self, expr: &Expr) -> Value {
        match self.eval(expr, &self.global_scope) {
            EvalResult::Value(v) | EvalResult::Return(v) => v,
            EvalResult::Error(e) => { eprintln!("Runtime Error: {}", e); Value::Void }
        }
    }

    fn hoist(&self, expr: &Expr, scope: &Arc<dyn Scope>) {
        if let Expr::Block(stmts, _) = expr {
            for stmt in stmts {
                if let Expr::Assign(name, val_expr, _) = stmt {
                    if let Expr::FuncDef(..) = &**val_expr {
                        if let EvalResult::Value(val) = self.eval(val_expr, scope) {
                            scope.set(name, val);
                        }
                    } else if let Expr::Assign(inner_name, inner_val, _) = &**val_expr {
                        // Handle nested macro assignments
                        if let Expr::FuncDef(..) = &**inner_val {
                            if let EvalResult::Value(val) = self.eval(inner_val, scope) {
                                scope.set(inner_name, val);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn eval(&self, expr: &Expr, scope: &Arc<dyn Scope>) -> EvalResult {
        match expr {
            Expr::Block(stmts, _) => {
                self.hoist(expr, scope);
                let mut last = Value::Void;
                for stmt in stmts {
                    if let Expr::Assign(_, val_expr, _) = stmt {
                        if let Expr::FuncDef(..) = &**val_expr { continue; }
                        if let Expr::Assign(_, inner_val, _) = &**val_expr {
                            if let Expr::FuncDef(..) = &**inner_val { continue; }
                        }
                    }
                    match self.eval(stmt, scope) {
                        EvalResult::Value(v) => last = v,
                        other => return other,
                    }
                }
                EvalResult::Value(last)
            },
            Expr::Assign(name, val_expr, _) => {
                match self.eval(val_expr, scope) {
                    EvalResult::Value(v) => { scope.set(name, v.clone()); EvalResult::Value(v) },
                    other => other,
                }
            },
            Expr::Literal(val, _) => EvalResult::Value(val.clone()),
            Expr::Ident(name, is_core, _) => {
                let val = if *is_core { self.core_scope.get(name) } else { scope.get(name) };
                EvalResult::Value(val)
            },
            Expr::Field(obj_expr, field_name, _) => {
                match self.eval(obj_expr, scope) {
                    EvalResult::Value(Value::Object(map)) => {
                        EvalResult::Value(map.borrow().get(field_name).cloned().unwrap_or(Value::Void))
                    },
                    EvalResult::Value(Value::Array(vec)) if field_name == "length" => {
                        EvalResult::Value(Value::Number(vec.borrow().len() as f64))
                    },
                    EvalResult::Value(Value::String(s)) if field_name == "length" => {
                        EvalResult::Value(Value::Number(s.len() as f64))
                    },
                    EvalResult::Value(_) => EvalResult::Value(Value::Void),
                    other => other,
                }
            },
            Expr::FuncDef(params, body, _) => {
                EvalResult::Value(Value::Task(Arc::new(TaskValue::User {
                    name: "anonymous".into(),
                    params: params.clone(),
                    body: *body.clone(),
                    closure: scope.clone(),
                })))
            },
            Expr::FuncCall(target_expr, arg_exprs, _) => {
                match self.eval(target_expr, scope) {
                    EvalResult::Value(target) => {
                        let mut args = Vec::new();
                        for arg_expr in arg_exprs {
                            match self.eval(arg_expr, scope) {
                                EvalResult::Value(v) => args.push(v),
                                other => return other,
                            }
                        }
                        self.call(&target, args, scope)
                    },
                    other => other,
                }
            },
            Expr::UnOp(op, target, _) => {
                match self.eval(target, scope) {
                    EvalResult::Value(val) => {
                        match op.as_str() {
                            "!" => EvalResult::Value(if self.is_truthy(&val) { Value::Void } else { Value::Number(1.0) }),
                            "?" => EvalResult::Value(val),
                            "^" => EvalResult::Return(val),
                            _ => EvalResult::Value(Value::Void),
                        }
                    },
                    other => other,
                }
            },
            Expr::Object(fields, _) => {
                let mut map = HashMap::new();
                for (k, v_expr) in fields {
                    match self.eval(v_expr, scope) {
                        EvalResult::Value(v) => { map.insert(k.clone(), v); },
                        other => return other,
                    }
                }
                EvalResult::Value(Value::Object(Arc::new(RefCell::new(map))))
            },
            Expr::Array(items, _) => {
                let mut vec = Vec::new();
                for item_expr in items {
                    match self.eval(item_expr, scope) {
                        EvalResult::Value(v) => vec.push(v),
                        other => return other,
                    }
                }
                EvalResult::Value(Value::Array(Arc::new(RefCell::new(vec))))
            },
            Expr::FlowControl { condition, success, fallback, rescue, catch_var, .. } => {
                match self.eval(condition, scope) {
                    EvalResult::Value(cond_val) => {
                        let res = if self.is_truthy(&cond_val) {
                            self.eval(success, scope)
                        } else if let Some(fb) = fallback {
                            self.eval(fb, scope)
                        } else { EvalResult::Value(Value::Void) };

                        if let EvalResult::Error(err_msg) = res {
                            if let Some(rescue_block) = rescue {
                                let rescue_scope: Arc<dyn Scope> = Arc::new(HALScope {
                                    values: RefCell::new(HashMap::new()),
                                    parent: Some(scope.clone()),
                                });
                                if let Some(var) = catch_var { rescue_scope.set(var, Value::String(err_msg)); }
                                self.eval(rescue_block, &rescue_scope)
                            } else { EvalResult::Error(err_msg) }
                        } else { res }
                    },
                    other => other,
                }
            },
        }
    }

    pub fn call(&self, task: &Value, args: Vec<Value>, scope: &Arc<dyn Scope>) -> EvalResult {
        if let Value::Task(tv) = task {
            match &**tv {
                TaskValue::Native { func, .. } => {
                    let ctx = HALExecutionContext { interp: self, scope: scope.clone() };
                    EvalResult::Value(func(args, &ctx))
                },
                TaskValue::User { params, body, closure, .. } => {
                    if args.len() > params.len() { return EvalResult::Error("Too many arguments".into()); }
                    let task_scope: Arc<dyn Scope> = Arc::new(HALScope {
                        values: RefCell::new(HashMap::new()),
                        parent: Some(closure.clone()),
                    });
                    for (i, p) in params.iter().enumerate() {
                        let val = if i < args.len() { args[i].clone() }
                        else if let Some(def_expr) = &p.default_value {
                            match self.eval(def_expr, &task_scope) {
                                EvalResult::Value(v) => v,
                                other => return other,
                            }
                        }
                        else if p.is_optional { Value::Void }
                        else { return EvalResult::Error(format!("Missing required argument: {}", p.name)); };
                        task_scope.set(&p.name, val);
                    }
                    match self.eval(body, &task_scope) {
                        EvalResult::Return(v) | EvalResult::Value(v) => EvalResult::Value(v),
                        EvalResult::Error(e) => EvalResult::Error(e),
                    }
                }
            }
        } else { EvalResult::Error(format!("Target is not a function: {:?}", task)) }
    }

    fn is_truthy(&self, v: &Value) -> bool { !matches!(v, Value::Void) }
}

pub struct HALExecutionContext<'a> { pub interp: &'a Interpreter, pub scope: Arc<dyn Scope> }

impl<'a> ExecutionContext for HALExecutionContext<'a> {
    fn call(&self, task: &Value, args: Vec<Value>) -> Value {
        match self.interp.call(task, args, &self.scope) {
            EvalResult::Value(v) | EvalResult::Return(v) => v,
            EvalResult::Error(e) => { eprintln!("Dynamic Call Error: {}", e); Value::Void }
        }
    }
    fn eval(&self, node: &Expr) -> Value {
        match self.interp.eval(node, &self.scope) {
            EvalResult::Value(v) | EvalResult::Return(v) => v,
            EvalResult::Error(e) => { eprintln!("Dynamic Eval Error: {}", e); Value::Void }
        }
    }
    fn scope(&self) -> &Arc<dyn Scope> { &self.scope }
}
