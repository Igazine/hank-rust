use crate::types::{HankError, HankErrorValue};
use std::collections::HashMap;

pub struct HankErrorRegistry;

impl HankErrorRegistry {
    pub fn get_messages() -> HashMap<HankError, String> {
        let mut m = HashMap::new();
        m.insert(HankError::UnexpectedCharacter, "Unexpected character: {0}".into());
        m.insert(HankError::UnclosedStringLiteral, "Unclosed string literal".into());
        
        m.insert(HankError::EmptyScript, "Syntax Error: Script is empty.".into());
        m.insert(HankError::ExpectedMainTask, "Syntax Error: Expected main task definition (a closure or a block).".into());
        m.insert(HankError::UnexpectedCodeOutsideMainTask, "Syntax Error: Unexpected code outside of main task. A Hank script must contain exactly one Task definition.".into());
        m.insert(HankError::InvalidAssignmentTarget, "Invalid assignment target".into());
        m.insert(HankError::UnexpectedToken, "Unexpected token: {0} ({1})".into());
        m.insert(HankError::MacroRequiresString, "Syntax Error: The '@' macro strictly requires a string literal path (e.g., @ \"utils\"). Identifier shorthand is not allowed.".into());
        m.insert(HankError::ExpectedIdentifier, "Expected identifier, found {0}".into());
        
        m.insert(HankError::CircularDependency, "Circular Dependency: {0}".into());
        m.insert(HankError::ResourceContentNotLoaded, "Resource content not loaded: {0}".into());
        m.insert(HankError::ScriptMustBeTask, "Hank Error: Script must evaluate to a Task definition.".into());
        m.insert(HankError::MacroResourceNotFound, "Macro resource not found: @{0}".into());
        
        m.insert(HankError::TargetNotFunction, "Target is not a function: {0}".into());
        m.insert(HankError::TooManyArguments, "Too many arguments".into());
        m.insert(HankError::MissingRequiredParameter, "Missing required parameter: {0}".into());
        m.insert(HankError::Halt, "HANK_HALT:{0}".into());
        m.insert(HankError::BitwiseOutOfBounds, "Value exceeds safe integer bounds for bitwise operation: {0}".into());
        m.insert(HankError::GenericRuntimeError, "{0}".into());
        m.insert(HankError::TypeMismatch, "Type Mismatch: Expected {0}, got {1} in {2}".into());
        m
    }

    pub fn create(code: HankError, args: Vec<String>, filename: Option<&str>, line: Option<usize>, line_text: Option<&str>) -> HankErrorValue {
        let messages = Self::get_messages();
        let mut tmpl = messages.get(&code).cloned().unwrap_or_else(|| "Unknown Error".into());

        for (i, arg) in args.iter().enumerate() {
            tmpl = tmpl.replace(&format!("{{{}}}", i), arg);
        }

        if let (Some(f), Some(l), Some(lt)) = (filename, line, line_text) {
            tmpl = format!("ERROR: {} in {} at\n\t{}:\t{}", tmpl, f, l, lt);
        }

        HankErrorValue { code, message: tmpl }
    }
}
