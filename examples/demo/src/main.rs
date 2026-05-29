use hank::types::{Value, ValueType, Expr, Resource, Arc};
use hank::runner::Runner;
use hank::stdlib;
use hank::ext::platform::PlatformExtension;
use hank::ext::sys::SysExtension;
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

    // Register Extensions (Batteries included, but disconnected)
    runner.register_extension(Box::new(stdlib::StdLib));
    runner.register_extension(Box::new(PlatformExtension));
    runner.register_extension(Box::new(SysExtension));

    runner
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
        // "test/conformance/14_syslib_hank.hank", // MOVED to extensions
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

    // Run Extension Tests
    let ext_tests = [
        "test/extensions/sys.hank",
        "test/extensions/platform_bin.hank",
    ];

    for t in &ext_tests {
        println!("--- Running Extension Test: {} ---", t);
        let runner = create_runner();
        let path = root.join(t);
        let abs_path = match fs::canonicalize(&path) {
            Ok(p) => p,
            Err(_) => { println!("Test not found: {}", path.display()); continue; }
        };
        let res = Arc::new(FileResource::new(abs_path.to_string_lossy().to_string()));
        if let Err(e) = runner.run(res, vec![]) {
            println!("Extension Test Failed: {}", e);
        }
        println!("--------------------\n");
    }
}
