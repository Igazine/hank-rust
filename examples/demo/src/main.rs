use hal::{Value, Runner};
use std::collections::HashMap;
use std::sync::Arc;
use std::cell::RefCell;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root = std::env::current_dir().unwrap();
    let workspace_root = root.join("../../vendor/hal");

    if args.len() < 2 {
        run_conformance(&workspace_root);
        return;
    }

    let mut runner = create_runner();
    
    let hal_args: Vec<Value> = args[2..].iter().map(|a| Value::String(a.clone())).collect();
    match runner.run(&args[1], hal_args) {
        Ok(val) => {
            if let Value::Number(n) = val { std::process::exit(n as i32); }
            std::process::exit(0);
        },
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn create_runner() -> Runner {
    // 1. Instantiate the core Runner with OS-specific I/O closures
    let read_file = Arc::new(|path: &str| {
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    });

    let resolve_path = Arc::new(|m: &str, base_file: &str| {
        let p = Path::new(m);
        if p.is_absolute() { 
            return p.canonicalize()
                .map(|cp| cp.to_string_lossy().to_string())
                .map_err(|e| format!("Failed to canonicalize {}: {}", m, e));
        }

        let base_dir = if base_file.is_empty() {
            std::env::current_dir().unwrap()
        } else {
            Path::new(base_file).parent().unwrap().to_path_buf()
        };

        let joined = base_dir.join(m);
        let mut final_path = joined.clone();
        if joined.extension().is_none() {
            let with_hal = joined.with_extension("hal");
            if with_hal.exists() { final_path = with_hal; }
        }

        final_path.canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|e| format!("Failed to resolve {}: {}", m, e))
    });

    let mut runner = Runner::new(read_file, resolve_path);

    // 2. Register the Standard Library manually (Optional)
    let std_modules = hal::stdlib::get_modules();
    for (name, tasks) in std_modules {
        runner.register_module(&name, tasks);
    }

    // 3. Register Custom SYSLIB
    register_syslib(&mut runner);

    runner
}

fn register_syslib(runner: &mut Runner) {
    let mut os_mod = HashMap::new();
    os_mod.insert("type".into(), Value::Task(Arc::new(hal::TaskValue::Native {
        name: "os.type".into(),
        func: |_, _| Value::String(std::env::consts::OS.into())
    })));
    os_mod.insert("memory".into(), Value::Task(Arc::new(hal::TaskValue::Native {
        name: "os.memory".into(),
        func: |_, _| {
            let mut map = HashMap::new();
            map.insert("total".into(), Value::Number(1024.0));
            map.insert("free".into(), Value::Number(512.0));
            Value::Object(Arc::new(RefCell::new(map)))
        }
    })));
    os_mod.insert("cpu".into(), Value::Task(Arc::new(hal::TaskValue::Native {
        name: "os.cpu".into(),
        func: |_, _| Value::Number(0.0)
    })));
    runner.register_module("os", os_mod);

    let mut host_mod = HashMap::new();
    host_mod.insert("cwd".into(), Value::Task(Arc::new(hal::TaskValue::Native {
        name: "host.cwd".into(),
        func: |_, _| Value::String(std::env::current_dir().unwrap().to_string_lossy().into())
    })));
    host_mod.insert("pid".into(), Value::Task(Arc::new(hal::TaskValue::Native {
        name: "host.pid".into(),
        func: |_, _| Value::Number(std::process::id() as f64)
    })));
    host_mod.insert("isRoot".into(), Value::Task(Arc::new(hal::TaskValue::Native {
        name: "host.isRoot".into(),
        func: |_, _| Value::Void
    })));
    host_mod.insert("signal".into(), Value::Task(Arc::new(hal::TaskValue::Native {
        name: "host.signal".into(),
        func: |args, _| {
            if let Some(arg0) = args.get(0) { println!("[SIGNAL] {}", val_to_string(arg0)); }
            Value::Void
        }
    })));
    runner.register_module("host", host_mod);

    let mut fs_mod = HashMap::new();
    fs_mod.insert("exists".into(), Value::Task(Arc::new(hal::TaskValue::Native {
        name: "fs.exists".into(),
        func: |args, _| {
            if let Some(Value::String(p)) = args.get(0) {
                if Path::new(p).exists() { return Value::Number(1.0); }
            }
            Value::Void
        }
    })));
    fs_mod.insert("read".into(), Value::Task(Arc::new(hal::TaskValue::Native {
        name: "fs.read".into(),
        func: |args, _| {
            if let Some(Value::String(p)) = args.get(0) {
                if let Ok(s) = std::fs::read_to_string(p) { return Value::String(s); }
            }
            Value::Void
        }
    })));
    fs_mod.insert("write".into(), Value::Task(Arc::new(hal::TaskValue::Native {
        name: "fs.write".into(),
        func: |args, _| {
            if let (Some(Value::String(p)), Some(Value::String(c))) = (args.get(0), args.get(1)) {
                if std::fs::write(p, c).is_ok() { return Value::Number(1.0); }
            }
            Value::Void
        }
    })));
    fs_mod.insert("deleteFile".into(), Value::Task(Arc::new(hal::TaskValue::Native {
        name: "fs.deleteFile".into(),
        func: |args, _| {
            if let Some(Value::String(p)) = args.get(0) {
                if std::fs::remove_file(p).is_ok() { return Value::Number(1.0); }
            }
            Value::Void
        }
    })));
    fs_mod.insert("stat".into(), Value::Task(Arc::new(hal::TaskValue::Native {
        name: "fs.stat".into(),
        func: |args, _| {
            if let Some(Value::String(p)) = args.get(0) {
                if let Ok(m) = std::fs::metadata(p) {
                    let mut map = HashMap::new();
                    map.insert("size".into(), Value::Number(m.len() as f64));
                    map.insert("mtime".into(), Value::Number(m.modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64));
                    map.insert("isDir".into(), if m.is_dir() { Value::Number(1.0) } else { Value::Void });
                    return Value::Object(Arc::new(RefCell::new(map)));
                }
            }
            Value::Void
        }
    })));
    runner.register_module("fs", fs_mod);

    let mut proc_mod = HashMap::new();
    proc_mod.insert("run".into(), Value::Task(Arc::new(hal::TaskValue::Native {
        name: "proc.run".into(),
        func: |args, _| {
            if let Some(Value::String(cmd)) = args.get(0) {
                let mut c = std::process::Command::new(cmd);
                if let Some(Value::Array(as_)) = args.get(1) {
                    for a in as_.borrow().iter() { c.arg(val_to_string(a)); }
                }
                if let Ok(out) = c.output() {
                    let mut map = HashMap::new();
                    map.insert("code".into(), Value::Number(out.status.code().unwrap_or(0) as f64));
                    map.insert("stdout".into(), Value::String(String::from_utf8_lossy(&out.stdout).into()));
                    map.insert("stderr".into(), Value::String(String::from_utf8_lossy(&out.stderr).into()));
                    return Value::Object(Arc::new(RefCell::new(map)));
                }
            }
            Value::Void
        }
    })));
    runner.register_module("proc", proc_mod);
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

fn run_conformance(workspace_root: &Path) {
    let tests = [
        "test/conformance/01_literals.hal",
        "test/conformance/02_gates.hal",
        "test/conformance/03_scoping.hal",
        "test/conformance/04_hoisting.hal",
        "test/conformance/05_params.hal",
        "test/conformance/06_macros.hal",
        "test/conformance/07_returns.hal",
        "test/conformance/08_host_args.hal",
        "test/conformance/09_deep_nesting.hal",
        "test/conformance/10_edge_cases.hal",
        "test/conformance/11_regex_parse.hal",
        "test/conformance/12_data_advanced.hal",
        "test/conformance/13_logic_module.hal",
        "test/conformance/14_syslib_hank.hal",
    ];

    for t in tests {
        println!("--- Running: {} ---", t);
        let mut runner = create_runner();
        let path = workspace_root.join(t);
        let args = if t.ends_with("08_host_args.hal") { vec![Value::String("Tamas".into())] } else { vec![] };
        match runner.run(&path.to_string_lossy(), args) {
            Ok(_) => {},
            Err(e) => println!("Test Failed: {}", e),
        }
        println!("--------------------\n");
    }
}
