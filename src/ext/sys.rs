use crate::types::{Value, NativeFunc, HankExtension, EvalResult, Arc, ValueType, ErrorValue, HankError, OpaqueValue};
use std::collections::HashMap;
use std::cell::RefCell;

pub struct SysExtension;

impl HankExtension for SysExtension {
    fn name(&self) -> &str { "SysExtension" }
    fn get_modules(&self) -> HashMap<String, HashMap<String, NativeFunc>> {
        let mut modules = HashMap::new();

        // --- host ---
        let mut host_mod = HashMap::new();
        host_mod.insert("cwd".into(), (|_, _| {
            let cwd = std::env::current_dir().unwrap_or_default().to_string_lossy().to_string();
            EvalResult::Value(Value::String(cwd))
        }) as NativeFunc);
        host_mod.insert("pid".into(), (|_, _| {
            EvalResult::Value(Value::Number(std::process::id() as f64))
        }) as NativeFunc);
        modules.insert("host".into(), host_mod);

        // --- os ---
        let mut os_mod = HashMap::new();
        os_mod.insert("type".into(), (|_, _| {
            EvalResult::Value(Value::String(std::env::consts::OS.to_string()))
        }) as NativeFunc);
        os_mod.insert("arch".into(), (|_, _| {
            EvalResult::Value(Value::String(std::env::consts::ARCH.to_string()))
        }) as NativeFunc);
        os_mod.insert("memory".into(), (|_, _| {
            let mut fields = HashMap::new();
            fields.insert("total".into(), Value::Number(0.0));
            fields.insert("free".into(), Value::Number(0.0));
            fields.insert("used".into(), Value::Number(0.0));
            EvalResult::Value(Value::Map(Arc::new(RefCell::new(fields))))
        }) as NativeFunc);
        os_mod.insert("cpu".into(), (|_, _| {
            EvalResult::Value(Value::Number(0.0))
        }) as NativeFunc);
        modules.insert("os".into(), os_mod);

        // --- fs ---
        let mut fs_mod = HashMap::new();
        fs_mod.insert("exists".into(), (|args, _| {
            if let Some(val) = args.get(0) {
                if let Value::String(path) = val {
                    if std::path::Path::new(path).exists() { return EvalResult::Value(Value::Number(1.0)); }
                } else {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", val.get_type())), Value::String("fs.exists".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
        fs_mod.insert("read".into(), (|args, _| {
            if let Some(val) = args.get(0) {
                if let Value::String(path) = val {
                    if let Ok(content) = std::fs::read_to_string(path) { return EvalResult::Value(Value::String(content)); }
                } else {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", val.get_type())), Value::String("fs.read".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
        fs_mod.insert("write".into(), (|args, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let path = match args.get(0).unwrap() {
                Value::String(s) => s,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", other.get_type())), Value::String("fs.write".into())] })))
            };
            let content = match args.get(1).unwrap() {
                Value::String(s) => s,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", other.get_type())), Value::String("fs.write".into())] })))
            };
            if std::fs::write(path, content).is_ok() { return EvalResult::Value(Value::Number(1.0)); }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
        fs_mod.insert("deleteFile".into(), (|args, _| {
            if let Some(val) = args.get(0) {
                if let Value::String(path) = val {
                    if std::fs::remove_file(path).is_ok() { return EvalResult::Value(Value::Number(1.0)); }
                } else {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", val.get_type())), Value::String("fs.deleteFile".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
        fs_mod.insert("stat".into(), (|args, _| {
            if let Some(val) = args.get(0) {
                if let Value::String(path) = val {
                    if let Ok(meta) = std::fs::metadata(path) {
                        let mut fields = HashMap::new();
                        fields.insert("size".into(), Value::Number(meta.len() as f64));
                        fields.insert("isDir".into(), if meta.is_dir() { Value::Number(1.0) } else { Value::Void });
                        if let Ok(mtime) = meta.modified() {
                            if let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH) {
                                fields.insert("mtime".into(), Value::Number(dur.as_millis() as f64));
                            }
                        }
                        return EvalResult::Value(Value::Map(Arc::new(RefCell::new(fields))));
                    }
                } else {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", val.get_type())), Value::String("fs.stat".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
        modules.insert("fs".into(), fs_mod);

        // --- proc ---
        let mut proc_mod = HashMap::new();
        proc_mod.insert("run".into(), (|args, _| {
            if let Some(val) = args.get(0) {
                if let Value::String(cmd) = val {
                    let mut command = std::process::Command::new(cmd);
                    if let Some(Value::Array(a)) = args.get(1) {
                        for arg in a.borrow().iter() {
                            command.arg(val_to_string(arg));
                        }
                    }
                    if let Ok(output) = command.output() {
                        let mut fields = HashMap::new();
                        fields.insert("code".into(), Value::Number(output.status.code().unwrap_or(1) as f64));
                        fields.insert("stdout".into(), Value::String(String::from_utf8_lossy(&output.stdout).to_string()));
                        fields.insert("stderr".into(), Value::String(String::from_utf8_lossy(&output.stderr).to_string()));
                        return EvalResult::Value(Value::Map(Arc::new(RefCell::new(fields))));
                    }
                } else {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", val.get_type())), Value::String("proc.run".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
        modules.insert("proc".into(), proc_mod);

        modules
    }
}

fn val_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Void => "Void".into(),
        Value::Array(_) => "[Array]".into(),
        Value::Map(_) => "[Map]".into(),
        Value::Opaque(ov) => format!("[Opaque:{}]", ov.label),
        Value::Task(_) => "[Task]".into(),
        Value::Error(e) => format!("[Error:{:?}]", e.code),
    }
}
