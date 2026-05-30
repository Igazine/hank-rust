use crate::types::{Value, NativeFunc, HankExtension, EvalResult, HankError, ValueType, ErrorValue, Arc};
use crate::error_registry::HankErrorRegistry;
use std::collections::HashMap;

const SAFE_INT_MAX: f64 = 9007199254740991.0;

fn check_safe_int(n: f64, task_name: &str) -> Result<i64, Value> {
    if n.abs() > SAFE_INT_MAX || !n.is_finite() {
        return Err(Value::Error(Arc::new(ErrorValue {
            code: HankError::BitwiseOutOfBounds,
            args: vec![Value::Number(n), Value::String(task_name.into())],
        })));
    }
    Ok(n as i64)
}

fn from_safe_int(n: i64, task_name: &str) -> Result<f64, Value> {
    let f = n as f64;
    if f.abs() > SAFE_INT_MAX {
        return Err(Value::Error(Arc::new(ErrorValue {
            code: HankError::BitwiseOutOfBounds,
            args: vec![Value::Number(f), Value::String(task_name.into())],
        })));
    }
    Ok(f)
}

pub struct PlatformExtension;

impl HankExtension for PlatformExtension {
    fn name(&self) -> &str { "PlatformExtension" }
    fn get_modules(&self) -> HashMap<String, HashMap<String, NativeFunc>> {
        let mut modules = HashMap::new();
        let mut bin_mod = HashMap::new();

        bin_mod.insert("and".into(), (|args, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let a = match args.get(0).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin.and".into())] })))
            };
            let b = match args.get(1).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin.and".into())] })))
            };
            let ia = match check_safe_int(a, "bin.and") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            let ib = match check_safe_int(b, "bin.and") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia & ib, "bin.and") {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }) as NativeFunc);

        bin_mod.insert("or".into(), (|args, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let a = match args.get(0).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin.or".into())] })))
            };
            let b = match args.get(1).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin.or".into())] })))
            };
            let ia = match check_safe_int(a, "bin.or") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            let ib = match check_safe_int(b, "bin.or") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia | ib, "bin.or") {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }) as NativeFunc);

        bin_mod.insert("xor".into(), (|args, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let a = match args.get(0).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin.xor".into())] })))
            };
            let b = match args.get(1).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin.xor".into())] })))
            };
            let ia = match check_safe_int(a, "bin.xor") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            let ib = match check_safe_int(b, "bin.xor") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia ^ ib, "bin.xor") {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }) as NativeFunc);

        bin_mod.insert("not".into(), (|args, _| {
            if args.is_empty() { return EvalResult::Value(Value::Void); }
            let a = match args.get(0).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin.not".into())] })))
            };
            let ia = match check_safe_int(a, "bin.not") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(!ia, "bin.not") {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }) as NativeFunc);

        bin_mod.insert("shiftL".into(), (|args, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let a = match args.get(0).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin.shiftL".into())] })))
            };
            let b = match args.get(1).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin.shiftL".into())] })))
            };
            let ia = match check_safe_int(a, "bin.shiftL") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia << (b as u32), "bin.shiftL") {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }) as NativeFunc);

        bin_mod.insert("shiftR".into(), (|args, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let a = match args.get(0).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin.shiftR".into())] })))
            };
            let b = match args.get(1).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin.shiftR".into())] })))
            };
            let ia = match check_safe_int(a, "bin.shiftR") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia >> (b as i32), "bin.shiftR") {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }) as NativeFunc);

        modules.insert("bin".into(), bin_mod);
        modules
    }
}
