use hank::types::{Value, ValueType, Expr, Resource, Arc};
use hank::runner::Runner;
use hank::stdlib;
use std::collections::HashMap;
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

struct FileResource {
    id: String,
    content: RefCell<Option<String>>,
    ast: RefCell<Option<Expr>>,
}

impl FileResource {
    fn new(path: String) -> Self {
        Self {
            id: path,
            content: RefCell::new(None),
            ast: RefCell::new(None),
        }
    }
}

impl Resource for FileResource {
    fn id(&self) -> &str {
        &self.id
    }

    fn content(&self) -> Option<String> {
        self.content.borrow().clone()
    }

    fn ast(&self) -> Option<Expr> {
        self.ast.borrow().clone()
    }

    fn set_ast(&self, ast: Expr) {
        *self.ast.borrow_mut() = Some(ast);
    }

    fn load(&self) -> Result<(), String> {
        if self.content.borrow().is_some() { return Ok(()); }
        let s = fs::read_to_string(&self.id).map_err(|e| e.to_string())?;
        *self.content.borrow_mut() = Some(s);
        Ok(())
    }

    fn resolve(&self, id: &str) -> Result<Box<dyn Resource>, String> {
        let mut path = PathBuf::from(id);
        if !path.is_absolute() {
            let base_dir = Path::new(&self.id).parent().unwrap_or(Path::new("."));
            path = base_dir.join(id);
        }

        if path.extension().is_none() {
            let hank_path = path.with_extension("hank");
            if hank_path.exists() {
                path = hank_path;
            }
        }

        let abs_path = fs::canonicalize(path).map_err(|e| e.to_string())?;
        Ok(Box::new(FileResource::new(abs_path.to_string_lossy().to_string())))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let current_dir = std::env::current_dir().unwrap();
    
    // Submodule is at vendor/hank relative to the hank-rust root
    let mut root = current_dir.join("vendor/hank");
    if !root.exists() {
        root = current_dir.join("../../vendor/hank");
    }

    if args.len() < 2 {
        run_conformance(&root);
        return;
    }

    let runner = create_runner();
    let script_path = Path::new(&args[1]);
    let abs_path = fs::canonicalize(script_path).unwrap();
    let res = Arc::new(FileResource::new(abs_path.to_string_lossy().to_string()));

    let mut hank_args = vec![];
    for arg in &args[2..] {
        hank_args.push(Value::String(arg.clone()));
    }

    match runner.run(res, hank_args) {
        Ok(val) => {
            if let Value::Number(n) = val {
                std::process::exit(n as i32);
            }
        },
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

fn create_runner() -> Runner {
    let runner = Runner::new();

    // 1. Register Standard Library
    let std = stdlib::get_modules();
    for (name, tasks) in std {
        runner.register_module(&name, tasks);
    }

    // 2. Register Example SYSLIB
    register_syslib(&runner);

    runner
}

fn register_syslib(runner: &Runner) {
    let mut os_mod = HashMap::new();
    os_mod.insert("type".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "os.type".into(),
        func: |_, _| {
            let s = std::env::consts::OS;
            if s.contains("macos") || s.contains("darwin") { Value::String("darwin".into()) }
            else if s.contains("windows") { Value::String("windows".into()) }
            else if s.contains("linux") { Value::String("linux".into()) }
            else { Value::String(s.into()) }
        },
    })));
    os_mod.insert("name".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "os.name".into(),
        func: |_, _| Value::String(std::env::consts::OS.into()),
    })));
    os_mod.insert("arch".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "os.arch".into(),
        func: |_, _| Value::String(std::env::consts::ARCH.into()),
    })));
    os_mod.insert("memory".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "os.memory".into(),
        func: |_, _| {
            let mut map = HashMap::new();
            map.insert("total".into(), Value::Number(0.0));
            map.insert("free".into(), Value::Number(0.0));
            map.insert("used".into(), Value::Number(0.0));
            Value::Object(Arc::new(RefCell::new(map)))
        },
    })));
    os_mod.insert("cpu".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "os.cpu".into(),
        func: |_, _| Value::Number(0.0),
    })));
    runner.register_module("os", os_mod);

    let mut host_mod = HashMap::new();
    host_mod.insert("cwd".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "host.cwd".into(),
        func: |_, _| Value::String(std::env::current_dir().unwrap().to_string_lossy().to_string()),
    })));
    host_mod.insert("pid".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "host.pid".into(),
        func: |_, _| Value::Number(std::process::id() as f64),
    })));
    host_mod.insert("isRoot".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "host.isRoot".into(),
        func: |_, _| Value::Void,
    })));
    runner.register_module("host", host_mod);

    let mut fs_mod = HashMap::new();
    fs_mod.insert("exists".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "fs.exists".into(),
        func: |args, _| {
            let path = if let Some(Value::String(s)) = args.get(0) { s } else { return Value::Void; };
            if Path::new(path).exists() { Value::Number(1.0) } else { Value::Void }
        },
    })));
    fs_mod.insert("read".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "fs.read".into(),
        func: |args, _| {
            let path = if let Some(Value::String(s)) = args.get(0) { s } else { return Value::Void; };
            fs::read_to_string(path).map(Value::String).unwrap_or(Value::Void)
        },
    })));
    fs_mod.insert("write".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "fs.write".into(),
        func: |args, _| {
            let path = if let Some(Value::String(s)) = args.get(0) { s } else { return Value::Void; };
            let content = if let Some(Value::String(s)) = args.get(1) { s } else { return Value::Void; };
            if fs::write(path, content).is_ok() { Value::Number(1.0) } else { Value::Void }
        },
    })));
    fs_mod.insert("deleteFile".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "fs.deleteFile".into(),
        func: |args, _| {
            let path = if let Some(Value::String(s)) = args.get(0) { s } else { return Value::Void; };
            if fs::remove_file(path).is_ok() { Value::Number(1.0) } else { Value::Void }
        },
    })));
    fs_mod.insert("stat".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "fs.stat".into(),
        func: |args, _| {
            let path = if let Some(Value::String(s)) = args.get(0) { s } else { return Value::Void; };
            if let Ok(m) = fs::metadata(path) {
                let mut map = HashMap::new();
                map.insert("size".into(), Value::Number(m.len() as f64));
                map.insert("mtime".into(), Value::Number(m.modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64));
                map.insert("isDir".into(), if m.is_dir() { Value::Number(1.0) } else { Value::Void });
                Value::Object(Arc::new(RefCell::new(map)))
            } else { Value::Void }
        },
    })));
    runner.register_module("fs", fs_mod);

    let mut proc_mod = HashMap::new();
    proc_mod.insert("run".into(), Value::Task(Arc::new(hank::types::TaskValue::Native {
        name: "proc.run".into(),
        func: |args, _| {
            let cmd_name = if let Some(Value::String(s)) = args.get(0) { s } else { return Value::Void; };
            let mut cmd = std::process::Command::new(cmd_name);
            if let Some(Value::Array(a)) = args.get(1) {
                for arg in a.borrow().iter() {
                    match arg {
                        Value::String(s) => { cmd.arg(s); },
                        Value::Number(n) => { cmd.arg(n.to_string()); },
                        _ => {}
                    }
                }
            }
            if let Ok(output) = cmd.output() {
                let mut map = HashMap::new();
                map.insert("code".into(), Value::Number(output.status.code().unwrap_or(0) as f64));
                map.insert("stdout".into(), Value::String(String::from_utf8_lossy(&output.stdout).to_string()));
                map.insert("stderr".into(), Value::String(String::from_utf8_lossy(&output.stderr).to_string()));
                Value::Object(Arc::new(RefCell::new(map)))
            } else { Value::Void }
        },
    })));
    runner.register_module("proc", proc_mod);
}

fn run_conformance(root: &Path) {
    let tests = [
        "test/conformance/01_literals.hank",
        "test/conformance/02_gates.hank",
        "test/conformance/03_scoping.hank",
        "test/conformance/04_hoisting.hank",
        "test/conformance/05_params.hank",
        "test/conformance/06_macros.hank",
        "test/conformance/07_returns.hank",
        "test/conformance/08_host_args.hank",
        "test/conformance/09_deep_nesting.hank",
        "test/conformance/10_edge_cases.hank",
        "test/conformance/11_regex_parse.hank",
        "test/conformance/12_data_advanced.hank",
        "test/conformance/13_logic_module.hank",
        "test/conformance/14_syslib_hank.hank",
        "test/conformance/15_logic_eq.hank",
        "test/conformance/16_chained_assign.hank",
        "test/conformance/17_num_module.hank",
    ];

    for t in &tests {
        println!("--- Running: {} ---", t);
        let runner = create_runner();
        let path = root.join(t);
        let abs_path = match fs::canonicalize(&path) {
            Ok(p) => p,
            Err(_) => { println!("Test not found: {}", path.display()); continue; }
        };
        let res = Arc::new(FileResource::new(abs_path.to_string_lossy().to_string()));
        
        let mut args = vec![];
        if t.ends_with("08_host_args.hank") {
            args.push(Value::String("Tamas".into()));
        }

        if let Err(e) = runner.run(res, args) {
            println!("Test Failed: {}", e);
        }
        println!("--------------------\n");
    }
}
