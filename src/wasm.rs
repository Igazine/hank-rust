#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use js_sys::Function;
#[cfg(target_arch = "wasm32")]
use crate::runner::Runner;
#[cfg(target_arch = "wasm32")]
use crate::types::Value;
#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WasmRunner {
    runner: Runner,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WasmRunner {
    #[wasm_bindgen(constructor)]
    pub fn new(read_file_js: Function, resolve_path_js: Function) -> Self {
        let rf_js = Arc::new(read_file_js);
        let rp_js = Arc::new(resolve_path_js);

        let read_file = Arc::new(move |path: &str| {
            let this = JsValue::NULL;
            let p = JsValue::from_str(path);
            match rf_js.call1(&this, &p) {
                Ok(val) => val.as_string().ok_or_else(|| "JS readFile did not return a string".to_string()),
                Err(e) => Err(format!("JS readFile error: {:?}", e)),
            }
        });

        let resolve_path = Arc::new(move |m: &str, base: &str| {
            let this = JsValue::NULL;
            let m_js = JsValue::from_str(m);
            let base_js = JsValue::from_str(base);
            match rp_js.call2(&this, &m_js, &base_js) {
                Ok(val) => val.as_string().ok_or_else(|| "JS resolvePath did not return a string".to_string()),
                Err(e) => Err(format!("JS resolvePath error: {:?}", e)),
            }
        });

        Self {
            runner: Runner::new(read_file, resolve_path),
        }
    }

    pub fn register_stdlib(&mut self) {
        let std = crate::stdlib::get_modules();
        for (name, tasks) in std {
            self.runner.register_module(&name, tasks);
        }
    }

    pub fn run(&mut self, script_path: &str) -> Result<String, JsValue> {
        match self.runner.run(script_path, vec![]) {
            Ok(val) => Ok(val_to_string(&val)),
            Err(e) => Err(JsValue::from_str(&e)),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn val_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Void => "null".into(),
        Value::Array(_) => "[Array]".into(),
        Value::Object(_) => "{Object}".into(),
        Value::Regex(_) => "[Regex]".into(),
        Value::Task(_) => "[Task]".into(),
    }
}
