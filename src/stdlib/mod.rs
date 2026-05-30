use crate::types::{Value, TaskValue, Scope, OpaqueValue, Arc, EvalResult, HankError, HankErrorValue, NativeFunc, HankExtension, ExecutionContext, Expr, ValueType, ErrorValue};
use crate::error_registry::HankErrorRegistry;
use std::collections::HashMap;
use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn wasm_log(s: &str);
}

fn hank_equals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Void, Value::Void) => true,
        (Value::Number(n1), Value::Number(n2)) => n1 == n2,
        (Value::String(s1), Value::String(s2)) => s1 == s2,
        (Value::Array(a1), Value::Array(a2)) => {
            let a1 = a1.borrow();
            let a2 = a2.borrow();
            if a1.len() != a2.len() { return false; }
            for i in 0..a1.len() {
                if !hank_equals(&a1[i], &a2[i]) { return false; }
            }
            true
        },
        (Value::Map(o1), Value::Map(o2)) => {
            let o1 = o1.borrow();
            let o2 = o2.borrow();
            if o1.len() != o2.len() { return false; }
            for (k, v1) in o1.iter() {
                if let Some(v2) = o2.get(k) {
                    if !hank_equals(v1, v2) { return false; }
                } else {
                    return false;
                }
            }
            true
        },
        (Value::Opaque(ov1), Value::Opaque(ov2)) => {
            ov1.label == ov2.label && Arc::ptr_eq(ov1, ov2)
        },
        (Value::Error(e1), Value::Error(e2)) => {
            if e1.code != e2.code || e1.args.len() != e2.args.len() { return false; }
            for i in 0..e1.args.len() {
                if !hank_equals(&e1.args[i], &e2.args[i]) { return false; }
            }
            true
        },
        _ => false,
    }
}

pub struct StdLib;

impl HankExtension for StdLib {
    fn name(&self) -> &str { "StdLib" }
    fn get_modules(&self) -> HashMap<String, HashMap<String, NativeFunc>> {
        get_stdlib_modules()
    }
}

pub fn get_stdlib_modules() -> HashMap<String, HashMap<String, NativeFunc>> {
    let mut modules = HashMap::new();

    // --- log ---
    let mut log_mod = HashMap::new();
    log_mod.insert("print".into(), (|args, _| {
            let msg = args.iter().map(|a| val_to_string(a)).collect::<Vec<_>>().join(" ");
            #[cfg(target_arch = "wasm32")]
            wasm_log(&msg);
            #[cfg(not(target_arch = "wasm32"))]
            println!("{}", msg);
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    log_mod.insert("error".into(), (|args, _| {
            let msg = args.iter().map(|a| val_to_string(a)).collect::<Vec<_>>().join(" ");
            #[cfg(target_arch = "wasm32")]
            wasm_log(&format!("[ERROR] {}", msg));
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("{}", msg);
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    log_mod.insert("warn".into(), (|args, _| {
            let msg = args.iter().map(|a| val_to_string(a)).collect::<Vec<_>>().join(" ");
            #[cfg(target_arch = "wasm32")]
            wasm_log(&format!("[WARN] {}", msg));
            #[cfg(not(target_arch = "wasm32"))]
            println!("[WARN] {}", msg);
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    modules.insert("log".into(), log_mod);

    // --- runtime ---
    let mut runtime_mod = HashMap::new();
    runtime_mod.insert("halt".into(), (|args, _| {
            let code = if let Some(Value::Number(n)) = args.get(0) { *n as i32 } else { 0 };
            std::process::exit(code);
        }) as NativeFunc);
    runtime_mod.insert("elapsedTime".into(), (|_ , _| EvalResult::Value(Value::Number(0.0))) as NativeFunc);
    runtime_mod.insert("signal".into(), (|args, _| {
            if !args.is_empty() {
                println!("[SIGNAL] {}", val_to_string(&args[0]));
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    modules.insert("runtime".into(), runtime_mod);

    // --- loop ---
    let mut loop_mod = HashMap::new();
    loop_mod.insert("while".into(), (|args, ctx| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let cond = &args[0];
            let body = &args[1];
            let mut last = Value::Void;
            loop {
                let cond_val = ctx.call(cond, vec![]);
                if ctx.is_error(&cond_val) { return EvalResult::Error(cond_val); }
                if matches!(cond_val, Value::Void) { break; }
                
                let res = ctx.call(body, vec![]);
                if let Value::Opaque(op) = &res {
                    if op.label == "__ControlFlow" && op.data.downcast_ref::<String>().map(|s| s == "Break").unwrap_or(false) {
                        break;
                    }
                }
                if ctx.is_error(&res) { return EvalResult::Error(res); }
                last = res;
            }
            EvalResult::Value(last)
        }) as NativeFunc);
    loop_mod.insert("break".into(), (|_, _| {
            EvalResult::Value(Value::Opaque(Arc::new(OpaqueValue { label: "__ControlFlow".into(), data: Box::new("Break".to_string()) })))
        }) as NativeFunc);
    modules.insert("loop".into(), loop_mod);

    // --- env ---
    let mut env_mod = HashMap::new();
    env_mod.insert("get".into(), (|_, _| EvalResult::Value(Value::Void)) as NativeFunc);
    env_mod.insert("set".into(), (|_, _| EvalResult::Value(Value::Void)) as NativeFunc);
    env_mod.insert("keys".into(), (|_, _| EvalResult::Value(Value::Array(Arc::new(RefCell::new(vec![]))))) as NativeFunc);
    modules.insert("env".into(), env_mod);

    // --- math ---
    let mut math_mod = HashMap::new();
    math_mod.insert("add".into(), (|args, _| {
            let mut sum = 0.0;
            for a in args {
                if let Value::Number(n) = a { sum += n; }
                else { return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", a.get_type())), Value::String("math.add".into())] }))); }
            }
            EvalResult::Value(Value::Number(sum))
        }) as NativeFunc);
    math_mod.insert("sub".into(), (|args, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            match (args.get(0).unwrap(), args.get(1).unwrap()) {
                (Value::Number(a), Value::Number(b)) => EvalResult::Value(Value::Number(a - b)),
                (a, b) => {
                    let faulty = if a.get_type() != ValueType::Number { a } else { b };
                    EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", faulty.get_type())), Value::String("math.sub".into())] })))
                }
            }
        }) as NativeFunc);
    math_mod.insert("mul".into(), (|args, _| {
            let mut res = 1.0;
            if args.is_empty() { return EvalResult::Value(Value::Number(0.0)); }
            for a in args {
                if let Value::Number(n) = a { res *= n; }
                else { return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", a.get_type())), Value::String("math.mul".into())] }))); }
            }
            EvalResult::Value(Value::Number(res))
        }) as NativeFunc);
    math_mod.insert("div".into(), (|args, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            match (args.get(0).unwrap(), args.get(1).unwrap()) {
                (Value::Number(a), Value::Number(b)) => {
                    if *b != 0.0 { EvalResult::Value(Value::Number(a / b)) } else { EvalResult::Value(Value::Void) }
                },
                (a, b) => {
                    let faulty = if a.get_type() != ValueType::Number { a } else { b };
                    EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", faulty.get_type())), Value::String("math.div".into())] })))
                }
            }
        }) as NativeFunc);
    math_mod.insert("gt".into(), (|args, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            match (args.get(0).unwrap(), args.get(1).unwrap()) {
                (Value::Number(a), Value::Number(b)) => EvalResult::Value(if a > b { Value::Number(1.0) } else { Value::Void }),
                (a, b) => {
                    let faulty = if a.get_type() != ValueType::Number { a } else { b };
                    EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", faulty.get_type())), Value::String("math.gt".into())] })))
                }
            }
        }) as NativeFunc);
    math_mod.insert("lt".into(), (|args, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            match (args.get(0).unwrap(), args.get(1).unwrap()) {
                (Value::Number(a), Value::Number(b)) => EvalResult::Value(if a < b { Value::Number(1.0) } else { Value::Void }),
                (a, b) => {
                    let faulty = if a.get_type() != ValueType::Number { a } else { b };
                    EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", faulty.get_type())), Value::String("math.lt".into())] })))
                }
            }
        }) as NativeFunc);
    math_mod.insert("eq".into(), (|args, _| { if let (Some(a), Some(b)) = (args.get(0), args.get(1)) { if hank_equals(a, b) { EvalResult::Value(Value::Number(1.0)) } else { EvalResult::Value(Value::Void) } } else { EvalResult::Value(Value::Void) } }) as NativeFunc);
    modules.insert("math".into(), math_mod);

    // --- str ---
    let mut str_mod = HashMap::new();
    str_mod.insert("length".into(), (|args, _| {
            if let Some(Value::String(s)) = args.get(0) { return EvalResult::Value(Value::Number(s.chars().count() as f64)); }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", other.get_type())), Value::String("str.length".into())] })));
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    str_mod.insert("format".into(), (|args, _| {
            if args.is_empty() { return EvalResult::Value(Value::Void); }
            let mut res = val_to_string(&args[0]);
            for i in 1..args.len() {
                res = res.replace(&format!("%{}", i), &val_to_string(&args[i]));
            }
            EvalResult::Value(Value::String(res))
        }) as NativeFunc);
    str_mod.insert("concat".into(), (|args, _| { EvalResult::Value(Value::String(args.iter().map(|a| val_to_string(a)).collect())) }) as NativeFunc);
    str_mod.insert("trim".into(), (|args, _| {
            if let Some(Value::String(s)) = args.get(0) { return EvalResult::Value(Value::String(s.trim().to_string())); }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", other.get_type())), Value::String("str.trim".into())] })));
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    modules.insert("str".into(), str_mod);

    // --- num ---
    let mut num_mod = HashMap::new();
    num_mod.insert("parse".into(), (|args, _| {
            if args.is_empty() { return EvalResult::Value(Value::Void); }
            let s = val_to_string(&args[0]);
            let mut base = if let Some(Value::Number(n)) = args.get(1) { *n as u32 } else { 0 };

            let final_s = if base == 0 {
                if s.starts_with("0x") { base = 16; &s[2..] }
                else if s.starts_with("0b") { base = 2; &s[2..] }
                else if s.starts_with("0o") { base = 8; &s[2..] }
                else { base = 10; &s }
            } else { &s };

            if let Ok(n) = i64::from_str_radix(final_s, base) {
                EvalResult::Value(Value::Number(n as f64))
            } else if base == 10 || base == 0 {
                if let Ok(f) = s.parse::<f64>() { EvalResult::Value(Value::Number(f)) } else { EvalResult::Value(Value::Void) }
            } else {
                EvalResult::Value(Value::Void)
            }
        }) as NativeFunc);
    num_mod.insert("format".into(), (|args, _| {
            if let Some(Value::Number(n)) = args.get(0) {
                let base = if let Some(Value::Number(b)) = args.get(1) { *b as u32 } else { 10 };
                if base < 2 || base > 36 { return EvalResult::Value(Value::Void); }
                let val = *n as i64;
                let chars = "0123456789abcdefghijklmnopqrstuvwxyz";
                if val == 0 { return EvalResult::Value(Value::String("0".into())); }
                let mut res = String::new();
                let mut curr = val.abs();
                while curr > 0 {
                    let rem = (curr % (base as i64)) as usize;
                    res.insert(0, chars.chars().nth(rem).unwrap());
                    curr /= base as i64;
                }
                if val < 0 { res.insert(0, '-'); }
                EvalResult::Value(Value::String(res))
            } else { EvalResult::Value(Value::Void) }
        }) as NativeFunc);
    modules.insert("num".into(), num_mod);

    // --- logic ---
    let mut logic_mod = HashMap::new();
    logic_mod.insert("and".into(), (|args, _| {
            if args.is_empty() { return EvalResult::Value(Value::Void); }
            let mut last = Value::Void;
            for a in args { if matches!(a, Value::Void) { return EvalResult::Value(Value::Void); } last = a.clone(); }
            EvalResult::Value(last)
        }) as NativeFunc);
    logic_mod.insert("or".into(), (|args, _| {
            for a in args { if !matches!(a, Value::Void) { return EvalResult::Value(a.clone()); } }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    logic_mod.insert("eq".into(), (|args, _| { if let (Some(a), Some(b)) = (args.get(0), args.get(1)) { if hank_equals(a, b) { EvalResult::Value(Value::Number(1.0)) } else { EvalResult::Value(Value::Void) } } else { EvalResult::Value(Value::Void) } }) as NativeFunc);
    modules.insert("logic".into(), logic_mod);

    // --- arr ---
    let mut arr_mod = HashMap::new();
    arr_mod.insert("length".into(), (|args, _| {
            if let Some(Value::Array(a)) = args.get(0) { return EvalResult::Value(Value::Number(a.borrow().len() as f64)); }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Array".into()), Value::String(format!("{:?}", other.get_type())), Value::String("arr.length".into())] })));
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    arr_mod.insert("get".into(), (|args, _| {
            if let (Some(Value::Array(a)), Some(Value::Number(n))) = (args.get(0), args.get(1)) {
                return EvalResult::Value(a.borrow().get(*n as usize).cloned().unwrap_or(Value::Void));
            }
            if let Some(other) = args.get(0) {
                if other.get_type() != ValueType::Array {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Array".into()), Value::String(format!("{:?}", other.get_type())), Value::String("arr.get".into())] })));
                }
            }
            if let Some(other) = args.get(1) {
                if other.get_type() != ValueType::Number {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("arr.get".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    arr_mod.insert("push".into(), (|args, _| {
            if let (Some(Value::Array(a)), Some(v)) = (args.get(0), args.get(1)) {
                a.borrow_mut().push(v.clone());
                return EvalResult::Value(Value::Void);
            }
            if let Some(other) = args.get(0) {
                if other.get_type() != ValueType::Array {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Array".into()), Value::String(format!("{:?}", other.get_type())), Value::String("arr.push".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    arr_mod.insert("pop".into(), (|args, _| {
            if let Some(Value::Array(a)) = args.get(0) { return EvalResult::Value(a.borrow_mut().pop().unwrap_or(Value::Void)); }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Array".into()), Value::String(format!("{:?}", other.get_type())), Value::String("arr.pop".into())] })));
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    arr_mod.insert("each".into(), (|args, ctx| {
            if let (Some(Value::Array(a)), Some(Value::Task(t))) = (args.get(0), args.get(1)) {
                let items = a.borrow().clone();
                for (idx, item) in items.iter().enumerate() {
                    let call_args = vec![item.clone(), Value::Number(idx as f64)];
                    let res = ctx.call(&Value::Task(t.clone()), call_args);
                    if let Value::Opaque(op) = &res {
                        if op.label == "__ControlFlow" && op.data.downcast_ref::<String>().map(|s| s == "Break").unwrap_or(false) {
                            break;
                        }
                    }
                    if ctx.is_error(&res) { return EvalResult::Error(res); }
                }
                return EvalResult::Value(Value::Void);
            }
            if let Some(other) = args.get(0) {
                if other.get_type() != ValueType::Array {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Array".into()), Value::String(format!("{:?}", other.get_type())), Value::String("arr.each".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    modules.insert("arr".into(), arr_mod);

    // --- map ---
    let mut map_mod = HashMap::new();
    map_mod.insert("get".into(), (|args, _| {
            if let (Some(Value::Map(m)), Some(k)) = (args.get(0), args.get(1)) {
                return EvalResult::Value(m.borrow().get(&val_to_string(k)).cloned().unwrap_or(Value::Void));
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    map_mod.insert("set".into(), (|args, _| {
            if args.len() < 3 { return EvalResult::Value(Value::Void); }
            if let (Some(Value::Map(m)), Some(k), Some(v)) = (args.get(0), args.get(1), args.get(2)) {
                m.borrow_mut().insert(val_to_string(k), v.clone());
                return EvalResult::Value(Value::Void);
            }
            if let Some(other) = args.get(0) {
                if other.get_type() != ValueType::Map {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Map".into()), Value::String(format!("{:?}", other.get_type())), Value::String("map.set".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    map_mod.insert("keys".into(), (|args, _| {
            if let Some(Value::Map(m)) = args.get(0) {
                let mut keys: Vec<Value> = m.borrow().keys().map(|k| Value::String(k.clone())).collect();
                keys.sort_by(|a, b| if let (Value::String(s1), Value::String(s2)) = (a, b) { s1.cmp(s2) } else { std::cmp::Ordering::Equal });
                EvalResult::Value(Value::Array(Arc::new(RefCell::new(keys))))
            } else { EvalResult::Value(Value::Void) }
        }) as NativeFunc);
    modules.insert("map".into(), map_mod);

    // --- json ---
    let mut json_mod = HashMap::new();
    json_mod.insert("parse".into(), (|args, _| {
            if let Some(Value::String(s)) = args.get(0) {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(s) { return EvalResult::Value(map_json_to_hank(data)); }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    json_mod.insert("stringify".into(), (|args, _| {
            if let Some(v) = args.get(0) {
                if let Some(j) = map_hank_to_json(v) {
                    if let Ok(s) = serde_json::to_string(&j) { return EvalResult::Value(Value::String(s)); }
                }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    modules.insert("json".into(), json_mod);

    // --- err ---
    let mut err_mod = HashMap::new();
    err_mod.insert("code".into(), (|args, _| {
            if let Some(Value::Error(e)) = args.get(0) { return EvalResult::Value(Value::Number(e.code as i32 as f64)); }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Error".into()), Value::String(format!("{:?}", other.get_type())), Value::String("err.code".into())] })));
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    err_mod.insert("message".into(), (|args, ctx| {
            if let Some(Value::Error(e)) = args.get(0) {
                let loc = ctx.get_localization();
                let mut msg = loc.get(&(e.code as i32)).cloned().unwrap_or_else(|| "Unknown Error".into());
                for (i, arg) in e.args.iter().enumerate() {
                    msg = msg.replace(&format!("{{{}}}", i), &val_to_string(arg));
                }
                return EvalResult::Value(Value::String(msg));
            }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Error".into()), Value::String(format!("{:?}", other.get_type())), Value::String("err.message".into())] })));
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    err_mod.insert("args".into(), (|args, _| {
            if let Some(Value::Error(e)) = args.get(0) { return EvalResult::Value(Value::Array(Arc::new(RefCell::new(e.args.clone())))); }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Error".into()), Value::String(format!("{:?}", other.get_type())), Value::String("err.args".into())] })));
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    err_mod.insert("isError".into(), (|args, _| {
            EvalResult::Value(if let Some(Value::Error(_)) = args.get(0) { Value::Number(1.0) } else { Value::Void })
        }) as NativeFunc);
    modules.insert("err".into(), err_mod);
    
    // --- regex ---
    let mut regex_mod = HashMap::new();
    regex_mod.insert("parse".into(), (|args, _| {
            if args.is_empty() { return EvalResult::Value(Value::Void); }
            let pattern = val_to_string(&args[0]);
            let flags = if args.len() > 1 { val_to_string(&args[1]) } else { "".into() };
            let mut final_pattern = pattern.clone();
            if flags.contains('i') { final_pattern = format!("(?i){}", final_pattern); }
            let re = regex_lite::Regex::new(&final_pattern).ok();
            if let Some(engine) = re {
                EvalResult::Value(Value::Opaque(Arc::new(OpaqueValue {
                    label: "RegExp".into(),
                    data: Box::new(engine),
                })))
            } else {
                EvalResult::Value(Value::Void)
            }
        }) as NativeFunc);
    regex_mod.insert("match".into(), (|args, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let s = val_to_string(&args[0]);
            match &args[1] {
                Value::Opaque(ov) if ov.label == "RegExp" => {
                    if let Some(re) = ov.data.downcast_ref::<regex_lite::Regex>() {
                        if re.is_match(&s) { return EvalResult::Value(Value::Number(1.0)); }
                    }
                }
                other => if s.contains(&val_to_string(other)) { return EvalResult::Value(Value::Number(1.0)); }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
    modules.insert("regex".into(), regex_mod);

    modules
}

fn val_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            let s = n.to_string();
            if s.ends_with(".0") { s[..s.len()-2].to_string() } else { s }
        },
        Value::Void => "Void".into(),
        Value::Array(_) => "[Array]".into(),
        Value::Map(_) => "[Map]".into(),
        Value::Opaque(ov) => format!("[Opaque:{}]", ov.label),
        Value::Task(_) => "[Task]".into(),
        Value::Error(e) => format!("[Error:{:?}]", e.code),
    }
}

fn map_json_to_hank(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Void,
        serde_json::Value::Bool(b) => if b { Value::Number(1.0) } else { Value::Void },
        serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(a) => Value::Array(Arc::new(RefCell::new(a.into_iter().map(map_json_to_hank).collect()))),
        serde_json::Value::Object(o) => {
            let mut map = HashMap::new();
            for (k, val) in o { map.insert(k, map_json_to_hank(val)); }
            Value::Map(Arc::new(RefCell::new(map)))
        }
    }
}

fn map_hank_to_json(v: &Value) -> Option<serde_json::Value> {
    match v {
        Value::Void => Some(serde_json::Value::Null),
        Value::Number(n) => Some(serde_json::Value::Number(serde_json::Number::from_f64(*n).unwrap())),
        Value::String(s) => Some(serde_json::Value::String(s.clone())),
        Value::Array(a) => {
            let mut items = vec![];
            for i in a.borrow().iter() {
                items.push(map_hank_to_json(i)?);
            }
            Some(serde_json::Value::Array(items))
        },
        Value::Map(o) => {
            let mut map = serde_json::Map::new();
            for (k, val) in o.borrow().iter() {
                map.insert(k.clone(), map_hank_to_json(val)?);
            }
            Some(serde_json::Value::Object(map))
        },
        Value::Opaque(_) => None,
        _ => Some(serde_json::Value::Null),
    }
}
