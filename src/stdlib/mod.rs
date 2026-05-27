use crate::types::{Value, TaskValue, Scope, OpaqueValue, Arc};
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

fn val_equals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Void, Value::Void) => true,
        (Value::Number(n1), Value::Number(n2)) => n1 == n2,
        (Value::String(s1), Value::String(s2)) => s1 == s2,
        (Value::Array(a1), Value::Array(a2)) => {
            let a1 = a1.borrow();
            let a2 = a2.borrow();
            if a1.len() != a2.len() { return false; }
            for i in 0..a1.len() {
                if !val_equals(&a1[i], &a2[i]) { return false; }
            }
            true
        },
        (Value::Object(o1), Value::Object(o2)) => {
            let o1 = o1.borrow();
            let o2 = o2.borrow();
            if o1.len() != o2.len() { return false; }
            for (k, v1) in o1.iter() {
                if let Some(v2) = o2.get(k) {
                    if !val_equals(v1, v2) { return false; }
                } else {
                    return false;
                }
            }
            true
        },
        (Value::Opaque(ov1), Value::Opaque(ov2)) => {
            ov1.label == ov2.label && Arc::ptr_eq(ov1, ov2)
        },
        _ => false,
    }
}

pub fn get_modules() -> HashMap<String, HashMap<String, Value>> {
    let mut modules = HashMap::new();

    // --- log ---
    let mut log_mod = HashMap::new();
    log_mod.insert("print".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "log.print".into(),
        func: |args, _| {
            let msg = args.iter().map(|a| val_to_string(a)).collect::<Vec<_>>().join(" ");
            #[cfg(target_arch = "wasm32")]
            wasm_log(&msg);
            #[cfg(not(target_arch = "wasm32"))]
            println!("{}", msg);
            Value::Void
        }
    })));
    log_mod.insert("error".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "log.error".into(),
        func: |args, _| {
            let msg = args.iter().map(|a| val_to_string(a)).collect::<Vec<_>>().join(" ");
            #[cfg(target_arch = "wasm32")]
            wasm_log(&format!("[ERROR] {}", msg));
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("{}", msg);
            Value::Void
        }
    })));
    log_mod.insert("warn".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "log.warn".into(),
        func: |args, _| {
            let msg = args.iter().map(|a| val_to_string(a)).collect::<Vec<_>>().join(" ");
            #[cfg(target_arch = "wasm32")]
            wasm_log(&format!("[WARN] {}", msg));
            #[cfg(not(target_arch = "wasm32"))]
            println!("[WARN] {}", msg);
            Value::Void
        }
    })));
    modules.insert("log".into(), log_mod);

    // --- runtime ---
    let mut runtime_mod = HashMap::new();
    runtime_mod.insert("halt".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "runtime.halt".into(),
        func: |args, _| {
            let code = if let Some(Value::Number(n)) = args.get(0) { *n as i32 } else { 0 };
            std::process::exit(code);
        }
    })));
    runtime_mod.insert("elapsedTime".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "runtime.elapsedTime".into(),
        func: |_, _| Value::Number(0.0)
    })));
    modules.insert("runtime".into(), runtime_mod);

    // --- env ---
    let mut env_mod = HashMap::new();
    env_mod.insert("get".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "env.get".into(),
        func: |_, _| Value::Void
    })));
    env_mod.insert("set".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "env.set".into(),
        func: |_, _| Value::Void
    })));
    env_mod.insert("keys".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "env.keys".into(),
        func: |_, _| Value::Array(Arc::new(RefCell::new(vec![])))
    })));
    modules.insert("env".into(), env_mod);

    // --- math ---
    let mut math_mod = HashMap::new();
    math_mod.insert("add".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "math.add".into(),
        func: |args, _| { Value::Number(args.iter().map(|a| if let Value::Number(n) = a { *n } else { 0.0 }).sum()) }
    })));
    math_mod.insert("sub".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "math.sub".into(),
        func: |args, _| { if let (Some(Value::Number(a)), Some(Value::Number(b))) = (args.get(0), args.get(1)) { Value::Number(a - b) } else { Value::Void } }
    })));
    math_mod.insert("mul".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "math.mul".into(),
        func: |args, _| { if args.is_empty() { Value::Number(0.0) } else { Value::Number(args.iter().map(|a| if let Value::Number(n) = a { *n } else { 1.0 }).product()) } }
    })));
    math_mod.insert("div".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "math.div".into(),
        func: |args, _| { if let (Some(Value::Number(a)), Some(Value::Number(b))) = (args.get(0), args.get(1)) { if *b != 0.0 { Value::Number(a / b) } else { Value::Void } } else { Value::Void } }
    })));
    math_mod.insert("gt".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "math.gt".into(),
        func: |args, _| { if let (Some(Value::Number(a)), Some(Value::Number(b))) = (args.get(0), args.get(1)) { if a > b { Value::Number(1.0) } else { Value::Void } } else { Value::Void } }
    })));
    math_mod.insert("lt".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "math.lt".into(),
        func: |args, _| { if let (Some(Value::Number(a)), Some(Value::Number(b))) = (args.get(0), args.get(1)) { if a < b { Value::Number(1.0) } else { Value::Void } } else { Value::Void } }
    })));
    math_mod.insert("eq".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "math.eq".into(),
        func: |args, _| { if let (Some(a), Some(b)) = (args.get(0), args.get(1)) { if val_equals(a, b) { Value::Number(1.0) } else { Value::Void } } else { Value::Void } }
    })));
    modules.insert("math".into(), math_mod);

    // --- str ---
    let mut str_mod = HashMap::new();
    str_mod.insert("length".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "str.length".into(),
        func: |args, _| {
            if let Some(Value::String(s)) = args.get(0) { return Value::Number(s.len() as f64); }
            Value::Void
        }
    })));
    str_mod.insert("format".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "str.format".into(),
        func: |args, _| {
            if args.is_empty() { return Value::Void; }
            let mut res = val_to_string(&args[0]);
            for i in 1..args.len() {
                res = res.replace(&format!("%{}", i), &val_to_string(&args[i]));
            }
            Value::String(res)
        }
    })));
    str_mod.insert("concat".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "str.concat".into(),
        func: |args, _| { Value::String(args.iter().map(|a| val_to_string(a)).collect()) }
    })));
    str_mod.insert("trim".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "str.trim".into(),
        func: |args, _| { if let Some(Value::String(s)) = args.get(0) { return Value::String(s.trim().to_string()); } Value::Void }
    })));
    modules.insert("str".into(), str_mod);

    // --- logic ---
    let mut logic_mod = HashMap::new();
    logic_mod.insert("and".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "logic.and".into(),
        func: |args, _| {
            if args.is_empty() { return Value::Void; }
            let mut last = Value::Void;
            for a in args { if matches!(a, Value::Void) { return Value::Void; } last = a.clone(); }
            last
        }
    })));
    logic_mod.insert("or".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "logic.or".into(),
        func: |args, _| {
            for a in args { if !matches!(a, Value::Void) { return a.clone(); } }
            Value::Void
        }
    })));
    logic_mod.insert("eq".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "logic.eq".into(),
        func: |args, _| { if let (Some(a), Some(b)) = (args.get(0), args.get(1)) { if val_equals(a, b) { Value::Number(1.0) } else { Value::Void } } else { Value::Void } }
    })));
    modules.insert("logic".into(), logic_mod);

    // --- arr ---
    let mut arr_mod = HashMap::new();
    arr_mod.insert("length".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "arr.length".into(),
        func: |args, _| { if let Some(Value::Array(a)) = args.get(0) { Value::Number(a.borrow().len() as f64) } else { Value::Void } }
    })));
    arr_mod.insert("get".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "arr.get".into(),
        func: |args, _| { if let (Some(Value::Array(a)), Some(Value::Number(n))) = (args.get(0), args.get(1)) { a.borrow().get(*n as usize).cloned().unwrap_or(Value::Void) } else { Value::Void } }
    })));
    arr_mod.insert("push".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "arr.push".into(),
        func: |args, _| { if let (Some(Value::Array(a)), Some(v)) = (args.get(0), args.get(1)) { a.borrow_mut().push(v.clone()); } Value::Void }
    })));
    arr_mod.insert("pop".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "arr.pop".into(),
        func: |args, _| { if let Some(Value::Array(a)) = args.get(0) { a.borrow_mut().pop().unwrap_or(Value::Void) } else { Value::Void } }
    })));
    arr_mod.insert("each".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "arr.each".into(),
        func: |args, ctx| {
            if let (Some(Value::Array(a)), Some(Value::Task(t))) = (args.get(0), args.get(1)) {
                let items = a.borrow().clone();
                for (idx, item) in items.iter().enumerate() {
                    let mut call_args = vec![item.clone(), Value::Number(idx as f64)];
                    if let TaskValue::User { params, .. } = &**t {
                        if call_args.len() > params.len() { call_args.truncate(params.len()); }
                    }
                    ctx.call(&Value::Task(t.clone()), call_args);
                }
            }
            Value::Void
        }
    })));
    modules.insert("arr".into(), arr_mod);

    // --- obj ---
    let mut obj_mod = HashMap::new();
    obj_mod.insert("get".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "obj.get".into(),
        func: |args, _| {
            if let (Some(Value::Object(m)), Some(k)) = (args.get(0), args.get(1)) {
                m.borrow().get(&val_to_string(k)).cloned().unwrap_or(Value::Void)
            } else { Value::Void }
        }
    })));
    obj_mod.insert("keys".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "obj.keys".into(),
        func: |args, _| {
            if let Some(Value::Object(m)) = args.get(0) {
                Value::Array(Arc::new(RefCell::new(m.borrow().keys().map(|k| Value::String(k.clone())).collect())))
            } else { Value::Void }
        }
    })));
    obj_mod.insert("values".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "obj.values".into(),
        func: |args, _| {
            if let Some(Value::Object(m)) = args.get(0) {
                Value::Array(Arc::new(RefCell::new(m.borrow().values().cloned().collect())))
            } else { Value::Void }
        }
    })));
    modules.insert("obj".into(), obj_mod);

    // --- json ---
    let mut json_mod = HashMap::new();
    json_mod.insert("parse".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "json.parse".into(),
        func: |args, _| {
            if let Some(Value::String(s)) = args.get(0) {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(s) { return map_json_to_hal(data); }
            }
            Value::Void
        }
    })));
    json_mod.insert("stringify".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "json.stringify".into(),
        func: |args, _| {
            if let Some(v) = args.get(0) {
                if let Some(j) = map_hal_to_json(v) {
                    if let Ok(s) = serde_json::to_string(&j) { return Value::String(s); }
                }
            }
            Value::Void
        }
    })));
    modules.insert("json".into(), json_mod);
    
    // --- regex ---
    let mut regex_mod = HashMap::new();
    regex_mod.insert("parse".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "regex.parse".into(),
        func: |args, _| {
            if args.is_empty() { return Value::Void; }
            let pattern = val_to_string(&args[0]);
            let flags = if args.len() > 1 { val_to_string(&args[1]) } else { "".into() };
            let mut final_pattern = pattern.clone();
            if flags.contains('i') { final_pattern = format!("(?i){}", final_pattern); }
            let re = regex_lite::Regex::new(&final_pattern).ok();
            if let Some(engine) = re {
                Value::Opaque(Arc::new(OpaqueValue {
                    label: "RegExp".into(),
                    data: Box::new(engine),
                }))
            } else {
                Value::Void
            }
        }
    })));
    regex_mod.insert("match".into(), Value::Task(Arc::new(TaskValue::Native {
        name: "regex.match".into(),
        func: |args, _| {
            if args.len() < 2 { return Value::Void; }
            let s = val_to_string(&args[0]);
            match &args[1] {
                Value::Opaque(ov) if ov.label == "RegExp" => {
                    if let Some(re) = ov.data.downcast_ref::<regex_lite::Regex>() {
                        if re.is_match(&s) { return Value::Number(1.0); }
                    }
                }
                other => if s.contains(&val_to_string(other)) { return Value::Number(1.0); }
            }
            Value::Void
        }
    })));
    modules.insert("regex".into(), regex_mod);

    modules
}

fn val_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Void => "null".into(),
        Value::Array(_) => "[Array]".into(),
        Value::Object(_) => "{Object}".into(),
        Value::Opaque(ov) => format!("[Opaque:{}]", ov.label),
        Value::Task(_) => "[Task]".into(),
    }
}

fn map_json_to_hal(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Void,
        serde_json::Value::Bool(b) => if b { Value::Number(1.0) } else { Value::Void },
        serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(a) => Value::Array(Arc::new(RefCell::new(a.into_iter().map(map_json_to_hal).collect()))),
        serde_json::Value::Object(o) => {
            let mut map = HashMap::new();
            for (k, val) in o { map.insert(k, map_json_to_hal(val)); }
            Value::Object(Arc::new(RefCell::new(map)))
        }
    }
}

fn map_hal_to_json(v: &Value) -> Option<serde_json::Value> {
    match v {
        Value::Void => Some(serde_json::Value::Null),
        Value::Number(n) => Some(serde_json::Value::Number(serde_json::Number::from_f64(*n).unwrap())),
        Value::String(s) => Some(serde_json::Value::String(s.clone())),
        Value::Array(a) => {
            let mut items = vec![];
            for i in a.borrow().iter() {
                items.push(map_hal_to_json(i)?);
            }
            Some(serde_json::Value::Array(items))
        },
        Value::Object(o) => {
            let mut map = serde_json::Map::new();
            for (k, val) in o.borrow().iter() {
                map.insert(k.clone(), map_hal_to_json(val)?);
            }
            Some(serde_json::Value::Object(map))
        },
        Value::Opaque(_) => None,
        _ => Some(serde_json::Value::Null),
    }
}
