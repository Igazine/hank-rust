use crate::types::{Expr, Value, TaskValue, ExecutionContext, Scope, Arc, HankError, HankErrorValue, EvalResult, ValueType, ErrorValue, OpaqueValue};
use crate::error_registry::HankErrorRegistry;
use std::collections::HashMap;
use std::cell::RefCell;

pub struct Interpreter {
    pub global_scope: Arc<dyn Scope>,
    pub core_scope: Arc<dyn Scope>,
    pub localization: HashMap<i32, String>,
    _depth: usize,
}

pub struct HankScope {
    pub values: RefCell<HashMap<String, Value>>,
    pub parent: Option<Arc<dyn Scope>>,
}

impl std::fmt::Debug for HankScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HankScope")
            .field("values", &self.values)
            .finish()
    }
}

impl HankScope {
    pub fn new(parent: Option<Arc<dyn Scope>>) -> Self {
        Self {
            values: RefCell::new(HashMap::new()),
            parent,
        }
    }
}

impl Scope for HankScope {
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

impl Interpreter {
    pub fn new(parent_scope: Option<Arc<dyn Scope>>, core_scope: Arc<dyn Scope>, localization: HashMap<i32, String>) -> Self {
        let global = parent_scope.unwrap_or_else(|| Arc::new(HankScope {
            values: RefCell::new(HashMap::new()),
            parent: Some(core_scope.clone()),
        }));
        Self { global_scope: global, core_scope, localization, _depth: 0 }
    }

    pub fn run(&mut self, expr: &Expr) -> Value {
        match self.eval_in_scope(expr, &self.global_scope) {
            EvalResult::Value(v) | EvalResult::Return(v) => v,
            EvalResult::Break => Value::Void,
            EvalResult::Error(v) => v
        }
    }

    pub fn is_truthy(&self, v: &Value) -> bool {
        !matches!(v, Value::Void)
    }

    pub fn is_error(&self, v: &Value) -> bool {
        matches!(v, Value::Error(_))
    }

    pub fn eval(&self, expr: &Expr, scope: &Arc<dyn Scope>) -> Value {
        match self.eval_in_scope(expr, scope) {
            EvalResult::Value(v) | EvalResult::Return(v) => v,
            EvalResult::Break => Value::Opaque(Arc::new(OpaqueValue { label: "__ControlFlow".into(), data: Box::new("Break".to_string()) })),
            EvalResult::Error(v) => v
        }
    }

    fn eval_in_scope(&self, expr: &Expr, scope: &Arc<dyn Scope>) -> EvalResult {
        const MAX_DEPTH: usize = 1000;
        if self._depth > MAX_DEPTH {
            return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::GenericRuntimeError, args: vec![Value::String("Stack overflow".into())] })));
        }

        match expr {
            Expr::Block(stmts, _) => {
                // --- TASK HOISTING PASS ---
                for stmt in stmts {
                    if let Expr::Assign(name, val_expr, _) = stmt {
                        if let Expr::FuncDef(_, _, _) = &**val_expr {
                            if let EvalResult::Value(v) = self.eval_in_scope(val_expr, scope) {
                                scope.set(name, v);
                            }
                        }
                        // Nested macro hoisting
                        if let Expr::Assign(inner_name, inner_val, _) = &**val_expr {
                            if let Expr::FuncDef(_, _, _) = &**inner_val {
                                if let EvalResult::Value(v) = self.eval_in_scope(inner_val, scope) {
                                    scope.set(inner_name, v);
                                }
                            }
                        }
                    }
                }

                let mut last = Value::Void;
                for stmt in stmts {
                    // Skip already hoisted tasks
                    if let Expr::Assign(_, val_expr, _) = stmt {
                        if let Expr::FuncDef(_, _, _) = &**val_expr { continue; }
                        if let Expr::Assign(_, inner_val, _) = &**val_expr {
                            if let Expr::FuncDef(_, _, _) = &**inner_val { continue; }
                        }
                    }

                    match self.eval_in_scope(stmt, scope) {
                        EvalResult::Value(v) => last = v,
                        other => return other,
                    }
                }
                EvalResult::Value(last)
            },
            Expr::Assign(name, val_expr, _) => {
                match self.eval_in_scope(val_expr, scope) {
                    EvalResult::Value(v) => { scope.set(name, v.clone()); EvalResult::Value(v) },
                    other => other,
                }
            },
            Expr::Literal(val, _) => EvalResult::Value(val.clone()),
            Expr::Ident(name, is_core, _) => {
                let val = if *is_core { self.core_scope.get(name) } else { 
                    let v = scope.get(name);
                    if v.get_type() == ValueType::Void { self.core_scope.get(name) } else { v }
                };
                EvalResult::Value(val)
            },
            Expr::Field(coll_expr, field_name, _) => {
                match self.eval_in_scope(coll_expr, scope) {
                    EvalResult::Value(Value::Map(map)) => {
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
                match self.eval_in_scope(target_expr, scope) {
                    EvalResult::Value(target) => {
                        let mut args = Vec::new();
                        for arg_expr in arg_exprs {
                            match self.eval_in_scope(arg_expr, scope) {
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
                match self.eval_in_scope(target, scope) {
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
            Expr::Map(fields, _) => {
                let mut map = HashMap::new();
                for (k, v_expr) in fields {
                    match self.eval_in_scope(v_expr, scope) {
                        EvalResult::Value(v) => { map.insert(k.clone(), v); },
                        other => return other,
                    }
                }
                EvalResult::Value(Value::Map(Arc::new(RefCell::new(map))))
            },
            Expr::Array(items, _) => {
                let mut vec = Vec::new();
                for item_expr in items {
                    match self.eval_in_scope(item_expr, scope) {
                        EvalResult::Value(v) => vec.push(v),
                        other => return other,
                    }
                }
                EvalResult::Value(Value::Array(Arc::new(RefCell::new(vec))))
            },
            Expr::Error(code, arg_exprs, _) => {
                let mut args = Vec::new();
                for arg_expr in arg_exprs {
                    match self.eval_in_scope(arg_expr, scope) {
                        EvalResult::Value(v) => args.push(v),
                        other => return other,
                    }
                }
                EvalResult::Value(Value::Error(Arc::new(ErrorValue { code: *code, args })))
            },
            Expr::FlowControl { condition, success, fallback, rescue, catch_var, .. } => {
                let cond_res = self.eval_in_scope(condition, scope);
                let branch_res = match cond_res {
                    EvalResult::Value(cond_val) => {
                        if self.is_truthy(&cond_val) {
                            self.eval_in_scope(success, scope)
                        } else if let Some(fb) = fallback {
                            self.eval_in_scope(fb, scope)
                        } else { EvalResult::Value(Value::Void) }
                    },
                    other => other,
                };

                match branch_res {
                    EvalResult::Error(err_val) if rescue.is_some() => {
                        let rescue_block = rescue.as_ref().unwrap();
                        let rescue_scope: Arc<dyn Scope> = Arc::new(HankScope {
                            values: RefCell::new(HashMap::new()),
                            parent: Some(scope.clone()),
                        });
                        if let Some(var) = catch_var { rescue_scope.set(var, err_val); }
                        self.eval_in_scope(rescue_block, &rescue_scope)
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
                    let ctx = HankExecutionContext { interp: self, scope: scope.clone() };
                    let res = func(args, &ctx);
                    match res {
                        EvalResult::Value(Value::Opaque(op)) if op.label == "__ControlFlow" && op.data.downcast_ref::<String>().map(|s| s == "Break").unwrap_or(false) => {
                            EvalResult::Break
                        },
                        _ => res
                    }
                },
                TaskValue::User { params, body, closure, .. } => {
                    if args.len() > params.len() {
                        return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TooManyArguments, args: vec![] })));
                    }
                    let task_scope: Arc<dyn Scope> = Arc::new(HankScope {
                        values: RefCell::new(HashMap::new()),
                        parent: Some(closure.clone()),
                    });
                    for (i, p) in params.iter().enumerate() {
                        let val = if i < args.len() { args[i].clone() }
                        else if let Some(def_expr) = &p.default_value {
                            match self.eval_in_scope(def_expr, &task_scope) {
                                EvalResult::Value(v) => v,
                                other => return other,
                            }
                        }
                        else if p.is_optional { Value::Void }
                        else {
                            return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::MissingRequiredParameter, args: vec![Value::String(p.name.clone())] })));
                        };
                        task_scope.set(&p.name, val);
                    }
                    let res = self.eval_in_scope(body, &task_scope);
                    match res {
                        EvalResult::Return(v) | EvalResult::Value(v) => {
                            if self.is_error(&v) { EvalResult::Error(v) } else { EvalResult::Value(v) }
                        },
                        other => other,
                    }
                }
            }
        } else {
            EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TargetNotFunction, args: vec![Value::String(format!("{:?}", task))] })))
        }
    }
}

pub struct HankExecutionContext<'a> {
    pub interp: &'a Interpreter,
    pub scope: Arc<dyn Scope>,
}

impl<'a> ExecutionContext for HankExecutionContext<'a> {
    fn call(&self, task: &Value, args: Vec<Value>) -> Value {
        match self.interp.call(task, args, &self.scope) {
            EvalResult::Value(v) | EvalResult::Return(v) => v,
            EvalResult::Break => Value::Opaque(Arc::new(OpaqueValue { label: "__ControlFlow".into(), data: Box::new("Break".to_string()) })),
            EvalResult::Error(v) => v,
        }
    }

    fn eval(&self, expr: &Expr) -> Value {
        match self.interp.eval_in_scope(expr, &self.scope) {
            EvalResult::Value(v) | EvalResult::Return(v) => v,
            EvalResult::Break => Value::Opaque(Arc::new(OpaqueValue { label: "__ControlFlow".into(), data: Box::new("Break".to_string()) })),
            EvalResult::Error(v) => v,
        }
    }

    fn is_error(&self, val: &Value) -> bool {
        self.interp.is_error(val)
    }

    fn get_localization(&self) -> HashMap<i32, String> {
        self.interp.localization.clone()
    }

    fn scope(&self) -> &Arc<dyn Scope> {
        &self.scope
    }
}
