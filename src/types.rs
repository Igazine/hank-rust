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
    Map,
    Opaque,
    Task,
    Error,
}

#[derive(Clone, Debug)]
pub enum Value {
    Void,
    Number(f64),
    String(String),
    Array(Arc<RefCell<Vec<Value>>>),
    Map(Arc<RefCell<HashMap<String, Value>>>),
    Opaque(Arc<OpaqueValue>),
    Task(Arc<TaskValue>),
    Error(Arc<ErrorValue>),
}

impl Value {
    pub fn get_type(&self) -> ValueType {
        match self {
            Self::Void => ValueType::Void,
            Self::Number(_) => ValueType::Number,
            Self::String(_) => ValueType::String,
            Self::Array(_) => ValueType::Array,
            Self::Map(_) => ValueType::Map,
            Self::Opaque(_) => ValueType::Opaque,
            Self::Task(_) => ValueType::Task,
            Self::Error(_) => ValueType::Error,
        }
    }
}

#[derive(Debug)]
pub struct OpaqueValue {
    pub label: String,
    pub data: Box<dyn std::any::Any + Send + Sync>,
}

#[derive(Debug, Clone)]
pub struct ErrorValue {
    pub code: HankError,
    pub args: Vec<Value>,
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

pub type NativeFunc = fn(args: Vec<Value>, ctx: &dyn ExecutionContext) -> EvalResult;

pub trait ExecutionContext {
    fn call(&self, task: &Value, args: Vec<Value>) -> Value;
    fn eval(&self, node: &Expr) -> Value;
    fn is_error(&self, val: &Value) -> bool;
    fn get_localization(&self) -> HashMap<i32, String>;
    fn scope(&self) -> &Arc<dyn Scope>;
}

pub trait Scope: std::fmt::Debug {
    fn get(&self, name: &str) -> Value;
    fn set(&self, name: &str, val: Value);
    fn exists(&self, name: &str) -> bool;
}

pub trait Resource {
    fn id(&self) -> &str;
    fn content(&self) -> Option<String>;
    fn ast(&self) -> Option<Expr>;
    fn set_ast(&self, ast: Expr);
    fn load(&self) -> Result<(), String>;
    fn resolve(&self, id: &str) -> Result<Box<dyn Resource>, String>;
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
    Map(HashMap<String, Expr>, TokenData),
    Array(Vec<Expr>, TokenData),
    FlowControl {
        condition: Box<Expr>,
        success: Box<Expr>,
        fallback: Option<Box<Expr>>,
        rescue: Option<Box<Expr>>,
        catch_var: Option<String>,
        token: TokenData,
    },
    Error(HankError, Vec<Expr>, TokenData),
}

#[derive(Clone, Debug, Default)]
pub struct TokenData {
    pub line: usize,
    pub line_text: String,
}

pub trait IHankSerializable {
    fn serialize_hal(&self) -> String;
}

pub trait HankExtension {
    fn name(&self) -> &str;
    fn get_modules(&self) -> HashMap<String, HashMap<String, NativeFunc>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HankError {
    // Lexical Errors (10xx)
    UnexpectedCharacter = 1001,
    UnclosedStringLiteral = 1002,

    // Syntax Errors (20xx)
    EmptyScript = 2001,
    ExpectedMainTask = 2002,
    UnexpectedCodeOutsideMainTask = 2003,
    InvalidAssignmentTarget = 2004,
    UnexpectedToken = 2005,
    MacroRequiresString = 2006,
    ExpectedIdentifier = 2007,

    // Resolution & Runner Errors (30xx)
    CircularDependency = 3001,
    ResourceContentNotLoaded = 3002,
    ScriptMustBeTask = 3003,
    MacroResourceNotFound = 3004,

    // Runtime Errors (40xx)
    TargetNotFunction = 4001,
    TooManyArguments = 4002,
    MissingRequiredParameter = 4003,
    Halt = 4004,
    BitwiseOutOfBounds = 4005,
    GenericRuntimeError = 4006,
    TypeMismatch = 4007,
}

#[derive(Debug, Clone)]
pub struct HankErrorValue {
    pub code: HankError,
    pub message: String,
}

impl std::fmt::Display for HankErrorValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for HankErrorValue {}

#[derive(Debug, Clone)]
pub enum EvalResult {
    Value(Value),
    Return(Value),
    Break,
    Error(Value), // Now Error is a Value (Type 8)
}
