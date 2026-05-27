use std::collections::HashMap;
use std::cell::RefCell;

#[cfg(not(target_arch = "wasm32"))]
pub use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
pub use std::rc::Rc as Arc;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ValueType {
    Void,
    Number,
    String,
    Array,
    Object,
    Opaque,
    Task,
}

#[derive(Clone, Debug)]
pub enum Value {
    Void,
    Number(f64),
    String(String),
    Array(Arc<RefCell<Vec<Value>>>),
    Object(Arc<RefCell<HashMap<String, Value>>>),
    Opaque(Arc<OpaqueValue>),
    Task(Arc<TaskValue>),
}

impl Value {
    pub fn get_type(&self) -> ValueType {
        match self {
            Self::Void => ValueType::Void,
            Self::Number(_) => ValueType::Number,
            Self::String(_) => ValueType::String,
            Self::Array(_) => ValueType::Array,
            Self::Object(_) => ValueType::Object,
            Self::Opaque(_) => ValueType::Opaque,
            Self::Task(_) => ValueType::Task,
        }
    }
}

#[derive(Debug)]
pub struct OpaqueValue {
    pub label: String,
    pub data: Box<dyn std::any::Any + Send + Sync>,
}

pub enum TaskValue {
    Native {
        name: String,
        func: NativeFunc,
    },
    User {
        name: String,
        params: Vec<Param>,
        body: Expr,
        closure: Arc<dyn Scope>,
    },
}

impl std::fmt::Debug for TaskValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Native { name, .. } => write!(f, "NativeTask({})", name),
            Self::User { name, .. } => write!(f, "UserTask({})", name),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub is_optional: bool,
    pub default_value: Option<Box<Expr>>,
}

pub type NativeFunc = fn(args: Vec<Value>, ctx: &dyn ExecutionContext) -> Value;

pub trait ExecutionContext {
    fn call(&self, task: &Value, args: Vec<Value>) -> Value;
    fn eval(&self, node: &Expr) -> Value;
    fn scope(&self) -> &Arc<dyn Scope>;
}

pub trait Scope: std::fmt::Debug {
    fn get(&self, name: &str) -> Value;
    fn set(&self, name: &str, val: Value);
    fn exists(&self, name: &str) -> bool;
}

#[derive(Clone, Debug)]
pub enum Expr {
    Block(Vec<Expr>, TokenData),
    Assign(String, Box<Expr>, TokenData),
    Literal(Value, TokenData),
    Ident(String, bool, TokenData),
    Field(Box<Expr>, String, TokenData),
    FuncDef(Vec<Param>, Box<Expr>, TokenData),
    FuncCall(Box<Expr>, Vec<Expr>, TokenData),
    UnOp(String, Box<Expr>, TokenData),
    Object(HashMap<String, Expr>, TokenData),
    Array(Vec<Expr>, TokenData),
    FlowControl {
        condition: Box<Expr>,
        success: Box<Expr>,
        fallback: Option<Box<Expr>>,
        rescue: Option<Box<Expr>>,
        catch_var: Option<String>,
        token: TokenData,
    },
}

#[derive(Clone, Debug, Default)]
pub struct TokenData {
    pub line: usize,
    pub line_text: String,
}

pub trait IHALSerializable {
    fn serialize_hal(&self) -> String;
}
