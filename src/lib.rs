use serde_json::Value;
use std::fmt::Debug;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FuzzyJsonError {
    #[error("Invalid JSON at position {pos}: {msg}")]
    ParseError { pos: usize, msg: String },
    #[error("Repair failed: {0}")]
    RepairFailed(String),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsonContext {
    Root,
    Object,
    Array,
    DoubleQuoteProperty,
    SingleQuoteProperty,
    DoubleQuoteValue,
    SingleQuoteValue,
    Colon,
}

impl JsonContext {
    pub fn is_value(&self) -> bool {
        self == &Self::DoubleQuoteValue || self == &Self::SingleQuoteValue
    }

    pub fn is_key(&self) -> bool {
        self == &Self::DoubleQuoteProperty || self == &Self::SingleQuoteProperty
    }
}

#[derive(Debug, Clone)]
pub struct ParseState {
    pub input: String,
    pub position: usize,
    pub stack: Vec<JsonContext>,
    pub output: String,
}

impl ParseState {
    pub fn new(input: String) -> Self {
        Self {
            input,
            position: 0,
            stack: vec![JsonContext::Root],
            output: String::new(),
        }
    }

    pub fn current_char(&self) -> Option<char> {
        self.input.chars().nth(self.position)
    }

    pub fn peek_chars(&self, count: usize) -> String {
        self.input.chars().skip(self.position).take(count).collect()
    }

    pub fn advance(&mut self, count: usize) -> String {
        let chars: String = self.input.chars().skip(self.position).take(count).collect();
        self.position += count;
        chars
    }

    pub fn remaining(&self) -> &str {
        match self
            .input
            .char_indices()
            .nth(self.position)
            .map(|(idx, _)| idx)
        {
            Some(start_byte) => &self.input[start_byte..],
            None => "",
        }
    }

    pub fn is_sq_key_or_value(&self) -> bool {
        let cc = self.current_context();

        cc == &JsonContext::SingleQuoteValue || cc == &JsonContext::SingleQuoteProperty
    }
    pub fn is_key_or_value(&self) -> bool {
        let cc = self.current_context();

        cc == &JsonContext::SingleQuoteValue
            || cc == &JsonContext::DoubleQuoteValue
            || cc == &JsonContext::SingleQuoteProperty
            || cc == &JsonContext::DoubleQuoteProperty
    }

    pub fn is_dq_key_or_value(&self) -> bool {
        let cc = self.current_context();

        cc == &JsonContext::DoubleQuoteValue || cc == &JsonContext::DoubleQuoteProperty
    }

    pub fn is_value(&self) -> bool {
        let cc = self.current_context();
        cc == &JsonContext::SingleQuoteValue || cc == &JsonContext::DoubleQuoteValue
    }

    pub fn is_prop(&self) -> bool {
        let cc = self.current_context();
        cc == &JsonContext::SingleQuoteProperty || cc == &JsonContext::DoubleQuoteProperty
    }

    pub fn is_finished(&self) -> bool {
        self.position >= self.input.chars().count()
    }

    pub fn current_context(&self) -> &JsonContext {
        self.stack.last().unwrap_or(&JsonContext::Root)
    }

    pub fn push_context(&mut self, context: JsonContext) {
        self.stack.push(context);
    }

    pub fn pop_context(&mut self) -> Option<JsonContext> {
        if self.stack.len() > 1 {
            self.stack.pop()
        } else {
            None
        }
    }
}

pub trait RepairStrategy: Send + Sync + Debug {
    fn name(&self) -> &'static str;
    fn can_repair(&self, state: &ParseState, error: &str) -> bool;
    fn repair(&self, state: &mut ParseState, error: &str) -> Result<(), FuzzyJsonError>;
    fn priority(&self) -> u8; // Higher priority strategies are tried first
}

pub trait StateHandler: Send + Sync + Debug {
    fn can_handle(&self, state: &ParseState) -> bool;
    fn handle(&self, state: &mut ParseState) -> Result<bool, FuzzyJsonError>; // Returns true if parsing should continue
}

#[derive(Default, Debug)]
pub struct FuzzyJsonParser {
    repair_strategies: Vec<Box<dyn RepairStrategy>>,
    state_handlers: Vec<Box<dyn StateHandler>>,
    options: ParserOptions,
}

#[derive(Debug, Clone)]
pub struct ParserOptions {
    pub auto_repair: bool,
    pub allow_trailing_commas: bool,
    pub allow_comments: bool,
    pub allow_single_quotes: bool,
    pub allow_unquoted_keys: bool,
    pub max_repair_attempts: usize,
    pub strict_mode: bool,
    pub aggressive_truncation_repair: bool, // New option for LLM truncation handling
}

impl Default for ParserOptions {
    fn default() -> Self {
        Self {
            auto_repair: true,
            allow_trailing_commas: true,
            allow_comments: true,
            allow_single_quotes: true,
            allow_unquoted_keys: false,
            max_repair_attempts: 1500,
            strict_mode: false,
            aggressive_truncation_repair: true, // Enable by default for LLM responses
        }
    }
}

impl FuzzyJsonParser {
    pub fn new() -> Self {
        let mut parser = Self::default();
        parser.register_default_strategies();
        parser.register_default_handlers();
        parser
    }

    /*
    /// Parse with aggressive scope closing for truncated LLM responses
    pub fn parse_with_auto_close<T>(&self, json_str: &str) -> Result<T, FuzzyJsonError>
    where
        T: serde::de::DeserializeOwned,
    {
        // First try normal parsing
        match self.parse_value(json_str) {
            Ok(value) => return serde_json::from_value(value).map_err(FuzzyJsonError::JsonError),
            Err(_) => {
                // Try with aggressive scope closing
                // println!("Try with aggressive scope closing");
                let closed_json = self.aggressively_close_scopes(json_str)?;
                let value = self.parse_value(&closed_json)?;
                serde_json::from_value(value).map_err(FuzzyJsonError::JsonError)
            }
        }
    }*/

    /// Aggressively close all unclosed scopes in potentially truncated JSON
    pub fn aggressively_close_scopes(&self, json_str: &str) -> Result<String, FuzzyJsonError> {
        if !self.options.aggressive_truncation_repair {
            return Ok(json_str.to_string());
        }
        let mut state = ParseState::new(json_str.trim().to_string());
        let mut in_string = false;
        let mut string_quote_char = '"';
        let mut escape_next = false;

        // Track unclosed scopes with their positions for better error reporting
        let mut scope_stack: Vec<(JsonContext, usize)> = vec![(JsonContext::Root, 0)];

        while !state.is_finished() {
            let ch = match state.current_char() {
                Some(c) => c,
                None => break,
            };

            if escape_next {
                state.output.push(ch);
                state.advance(1);
                escape_next = false;
                continue;
            }

            match ch {
                '\\' if in_string => {
                    state.output.push(ch);
                    state.advance(1);
                    escape_next = true;
                }
                '"' | '\'' if !in_string => {
                    in_string = true;
                    string_quote_char = ch;
                    state.output.push(ch);
                    state.advance(1);
                }
                c if in_string && c == string_quote_char => {
                    in_string = false;
                    state.output.push(ch);
                    state.advance(1);
                }
                '{' if !in_string => {
                    scope_stack.push((JsonContext::Object, state.position));
                    state.output.push(ch);
                    state.advance(1);
                }
                '[' if !in_string => {
                    scope_stack.push((JsonContext::Array, state.position));
                    state.output.push(ch);
                    state.advance(1);
                }
                '}' if !in_string => {
                    if let Some((JsonContext::Object, _)) = scope_stack.last() {
                        scope_stack.pop();
                    }
                    state.output.push(ch);
                    state.advance(1);
                }
                ']' if !in_string => {
                    if let Some((JsonContext::Array, _)) = scope_stack.last() {
                        scope_stack.pop();
                    }
                    state.output.push(ch);
                    state.advance(1);
                }
                _ => {
                    state.output.push(ch);
                    state.advance(1);
                }
            }
        }

        // Now aggressively close all unclosed scopes
        self.close_remaining_scopes(&mut state, in_string, string_quote_char, scope_stack)?;

        Ok(state.output)
    }

    fn close_remaining_scopes(
        &self,
        state: &mut ParseState,
        in_string: bool,
        string_quote_char: char,
        mut scope_stack: Vec<(JsonContext, usize)>,
    ) -> Result<(), FuzzyJsonError> {
        // First, close any unclosed string
        if in_string {
            state.output.push(string_quote_char);
            // in_string = false;
        }

        // Remove any trailing comma that might cause issues
        let trimmed_output = state.output.trim_end();
        if trimmed_output.ends_with(',') {
            state.output = trimmed_output[..trimmed_output.len() - 1].to_string();
        }

        // Close scopes in reverse order (LIFO)
        while let Some((context, _pos)) = scope_stack.pop() {
            match context {
                JsonContext::Object => {
                    state.output.push('}');
                }
                JsonContext::Array => {
                    state.output.push(']');
                }
                JsonContext::Root => {
                    // Don't close root context
                    break;
                }
                _ => {} // Other contexts don't need explicit closing
            }
        }

        Ok(())
    }

    pub fn with_options(options: ParserOptions) -> Self {
        let mut parser = Self {
            options,
            ..Default::default()
        };
        parser.register_default_strategies();
        parser.register_default_handlers();
        parser
    }

    pub fn register_strategy(&mut self, strategy: Box<dyn RepairStrategy>) {
        self.repair_strategies.push(strategy);
        // Sort by priority (highest first)
        self.repair_strategies
            .sort_by(|a, b| b.priority().cmp(&a.priority()));
    }

    pub fn register_handler(&mut self, handler: Box<dyn StateHandler>) {
        self.state_handlers.push(handler);
    }

    pub fn parse<T>(&self, json_str: &str) -> Result<T, FuzzyJsonError>
    where
        T: serde::de::DeserializeOwned,
    {
        let value = self.parse_value(json_str)?;
        serde_json::from_value(value).map_err(FuzzyJsonError::JsonError)
    }

    pub fn parse_value(&self, json_str: &str) -> Result<Value, FuzzyJsonError> {
        // First try standard parsing
        match serde_json::from_str(json_str) {
            Ok(value) => Ok(value),
            Err(e) => {
                if !self.options.auto_repair {
                    return Err(FuzzyJsonError::RepairFailed(
                        "Auto-repair disabled".to_string(),
                    ));
                }

                // Try fuzzy parsing with repair
                let repaired = self.repair_json(json_str, e)?;
                serde_json::from_str(&repaired).map_err(FuzzyJsonError::JsonError)
            }
        }
    }

    pub fn repair_json(
        &self,
        json_str: &str,
        e: serde_json::error::Error,
    ) -> Result<String, FuzzyJsonError> {
        let mut state = ParseState::new(json_str.trim().to_string());
        let mut attempts = 0;

        self.try_repair_strategies(&mut state, &e.to_string())?;
        // try repairing once
        /*
        println!(
            "Repair response : {:?} | State afterwards: {:?}",
            repair_successful, state
        );*/
        // .context("Failed to repair json using available repair strategies")?;

        while !state.is_finished() && attempts < self.options.max_repair_attempts {
            let mut handled = false;

            // Try state handlers first
            for handler in &self.state_handlers {
                if handler.can_handle(&state) {
                    /*
                    #[cfg(debug_assertions)]
                    println!(
                        "State: {:?} | {:?} : {:?} | {:?} | Handler: {:?} | Context: {:?}",
                        state.position,
                        state.current_char(),
                        state.remaining().chars().nth(0),
                        state.output,
                        handler,
                        state.current_context()
                    );*/
                    match handler.handle(&mut state) {
                        Ok(should_continue) => {
                            handled = true;
                            if !should_continue {
                                return Ok(state.output);
                            }
                            break;
                        }
                        Err(e) => {
                            // println!("State(e): {:?}", e);
                            // Try repair strategies
                            if self.try_repair_strategies(&mut state, &e.to_string())? {
                                handled = true;
                                break;
                            }
                        }
                    }
                } else {
                    /*
                    #[cfg(debug_assertions)]
                    println!(
                        "Can't handle |  handler: {:?} | State(e): {:?} | Remaining First Char: {:?} |  Current Char: {:?}",
                        handler,
                        state.position,
                        state.remaining().chars().nth(0),
                        state.current_char()
                    );*/
                }
            }
            if !handled {
                /* println!(
                    "Not handled |  output: {:?} | State(e): {:?} | Current Char: {:?}",
                    state.output,
                    state.position,
                    state.current_char()
                );*/
                if self.try_repair_strategies(&mut state, &e.to_string())? {
                    handled = true;
                }
            }

            if !handled {
                return Err(FuzzyJsonError::ParseError {
                    pos: state.position,
                    msg: format!(
                        "No handler for current state: {:?} | {:?}",
                        state.current_context(),
                        state.current_char()
                    ),
                });
            }

            attempts += 1;
        }
        /*
        println!(
            " Repaired so far: {:?} | End Context: {:?} | Current Char: {:?}",
            state.output,
            state.current_context(),
            state.current_char()
        );*/
        if state.current_context() != &JsonContext::Root {
            /*
            #[cfg(debug_assertions)]
            println!(
                "Repairing the case of incomplete json | Repaired so far: {:?} | End Context: {:?}",
                state.output,
                state.current_context()
            );*/
            self.try_repair_strategies(&mut state, &e.to_string())?;
        }

        if attempts >= self.options.max_repair_attempts {
            return Err(FuzzyJsonError::RepairFailed(
                "Too many repair attempts".to_string(),
            ));
        }

        // #[cfg(debug_assertions)]
        // println!("Output: {:?}", state.output);
        Ok(state.output)
    }

    fn try_repair_strategies(
        &self,
        state: &mut ParseState,
        error: &str,
    ) -> Result<bool, FuzzyJsonError> {
        // println!("COntext: {:?} | Is key: {:?}", state.current_context(), state.is_prop());
        for strategy in &self.repair_strategies {
            if strategy.can_repair(state, error) {
                // #[cfg(debug_assertions)]
                // println!("Repaired using {:?} | output: {}", strategy, state.output);
                strategy.repair(state, error)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn register_default_strategies(&mut self) {
        self.register_strategy(Box::new(TruncationRepairStrategy));
        self.register_strategy(Box::new(SingleQuotesStrategy));
        self.register_strategy(Box::new(CodeBlockMarkersStrategy));
        self.register_strategy(Box::new(IncompletePropertyStrategy));
        self.register_strategy(Box::new(IncompleteArrayStrategy));
        self.register_strategy(Box::new(TrailingCommaStrategy));
        self.register_strategy(Box::new(MissingQuotesStrategy));
        self.register_strategy(Box::new(MissingBracketsStrategy));
        self.register_strategy(Box::new(TrimStrayContentInBeginningStrategy));
        self.register_strategy(Box::new(TrimStrayContentInEndStrategy));
    }

    fn register_default_handlers(&mut self) {
        self.register_handler(Box::new(WhitespaceHandler));
        self.register_handler(Box::new(LiteralHandler));
        self.register_handler(Box::new(ColonHandler));
        self.register_handler(Box::new(CommaHandler));
        self.register_handler(Box::new(StringHandler));
        self.register_handler(Box::new(NumberHandler));
        self.register_handler(Box::new(ObjectHandler));
        self.register_handler(Box::new(ArrayHandler));
        self.register_handler(Box::new(NoQuotesKeyHandler));
    }
}

// Repair Strategies
#[derive(Debug)]
pub struct TrailingCommaStrategy;

impl RepairStrategy for TrailingCommaStrategy {
    fn name(&self) -> &'static str {
        "trailing_comma"
    }
    fn priority(&self) -> u8 {
        80
    }

    fn can_repair(&self, state: &ParseState, _error: &str) -> bool {
        if let Some(ch) = state.current_char() {
            ch == ','
                && state
                    .peek_chars(2)
                    .chars()
                    .nth(1)
                    .map_or(false, |next| next == '}' || next == ']')
        } else {
            false
        }
    }

    fn repair(&self, state: &mut ParseState, _error: &str) -> Result<(), FuzzyJsonError> {
        // Skip the trailing comma
        state.advance(1);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MissingQuotesStrategy;

// it only works for property keys and not for values
impl RepairStrategy for MissingQuotesStrategy {
    fn name(&self) -> &'static str {
        "missing_quotes"
    }
    fn priority(&self) -> u8 {
        70
    }

    fn can_repair(&self, state: &ParseState, error: &str) -> bool {
        error.contains("expected") && error.contains("quote")
            || (state.current_context() == &JsonContext::DoubleQuoteProperty
                && state.current_char().map_or(false, |c| c.is_alphabetic()))
    }

    fn repair(&self, state: &mut ParseState, _error: &str) -> Result<(), FuzzyJsonError> {
        println!("Repairing missing quotes");
        state.output.push(
            if state.current_context() == &JsonContext::SingleQuoteProperty {
                '\''
            } else {
                '"'
            },
        );

        // Collect until we hit a delimiter
        while let Some(ch) = state.current_char() {
            if ch.is_whitespace() || ch == ':' || ch == ',' || ch == '}' || ch == ']' {
                break;
            }
            state.output.push(ch);
            state.advance(1);
        }

        state.output.push(
            if state.current_context() == &JsonContext::SingleQuoteProperty {
                '\''
            } else {
                '"'
            },
        );
        Ok(())
    }
}

#[derive(Debug)]
pub struct MissingBracketsStrategy;

impl RepairStrategy for MissingBracketsStrategy {
    fn name(&self) -> &'static str {
        "missing_brackets"
    }
    fn priority(&self) -> u8 {
        60
    }

    fn can_repair(&self, _state: &ParseState, error: &str) -> bool {
        error.contains("missing") && (error.contains("}") || error.contains("]"))
    }

    fn repair(&self, state: &mut ParseState, error: &str) -> Result<(), FuzzyJsonError> {
        if error.contains("}") {
            state.output.push('}');
            state.pop_context();
        } else if error.contains("]") {
            state.output.push(']');
            state.pop_context();
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CodeBlockMarkersStrategy;

impl RepairStrategy for CodeBlockMarkersStrategy {
    fn name(&self) -> &'static str {
        "code_block_markers"
    }
    fn priority(&self) -> u8 {
        90
    }

    fn can_repair(&self, state: &ParseState, _error: &str) -> bool {
        state.remaining().starts_with("```") || state.remaining().starts_with("json```")
    }

    fn repair(&self, state: &mut ParseState, _error: &str) -> Result<(), FuzzyJsonError> {
        if state.remaining().starts_with("json```") {
            state.advance(7);
        } else if state.remaining().starts_with("```json") {
            state.advance(7);
        } else if state.remaining().starts_with("```") {
            state.advance(3);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct TrimStrayContentInBeginningStrategy;

impl RepairStrategy for TrimStrayContentInBeginningStrategy {
    fn name(&self) -> &'static str {
        "trim_stray_characters_in_end_markers"
    }
    fn priority(&self) -> u8 {
        70
    }

    fn can_repair(&self, state: &ParseState, _error: &str) -> bool {
        state.current_context() == &JsonContext::Root
            && (state.current_char() != Some('{') || state.current_char() != Some('['))
    }

    fn repair(&self, state: &mut ParseState, _error: &str) -> Result<(), FuzzyJsonError> {
        while state.current_char().is_some()
            && state.current_char() != Some('{')
            && state.current_char() != Some('[')
        {
            // this normally works for stray chars in end as well
            // but there could be stray `{` in the end as well // those will be captured/corrected
            // by the in the end strategy
            state.advance(1);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct TrimStrayContentInEndStrategy;

impl RepairStrategy for TrimStrayContentInEndStrategy {
    fn name(&self) -> &'static str {
        "trim_stray_characters_in_end_markers"
    }
    fn priority(&self) -> u8 {
        70
    }

    fn can_repair(&self, state: &ParseState, _error: &str) -> bool {
        state.current_context() == &JsonContext::Root
        //  && (state.current_char() != Some(']') || state.current_char() != Some('}'))
    }

    fn repair(&self, state: &mut ParseState, _error: &str) -> Result<(), FuzzyJsonError> {
        println!("Char: {:?}", state.current_char());
        while state.current_char() != None {
            state.advance(1);
            println!("Char: {:?}", state.current_char());
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SingleQuotesStrategy;

impl RepairStrategy for SingleQuotesStrategy {
    fn name(&self) -> &'static str {
        "single_quotes"
    }
    fn priority(&self) -> u8 {
        85 // higher than incomplete property strategy basically
    }

    fn can_repair(&self, state: &ParseState, _error: &str) -> bool {
        state.current_char() == Some('\'')
    }

    fn repair(&self, state: &mut ParseState, _error: &str) -> Result<(), FuzzyJsonError> {
        state.output.push('"');
        state.advance(1); // Skip the single quote

        while let Some(ch) = state.current_char() {
            if ch == '\'' {
                state.advance(1);
                break;
            }
            if ch == '"' {
                state.output.push('\\');
            }
            state.output.push(ch);
            state.advance(1);
        }
        if state.current_context() == &JsonContext::Colon {
            state.pop_context(); // for the cases when property was defined correctly and colon was
            // there but value
            // happened to be in single quotes
            // we are still missing the case where there's '' quote right
            // after property without any colons
        }

        state.output.push('"');
        Ok(())
    }
}

// High-priority strategy for handling LLM truncation
#[derive(Debug)]
pub struct TruncationRepairStrategy;

impl RepairStrategy for TruncationRepairStrategy {
    fn name(&self) -> &'static str {
        "truncation_repair"
    }
    fn priority(&self) -> u8 {
        95
    } // Highest priority

    fn can_repair(&self, state: &ParseState, error: &str) -> bool {
        // Detect if we're at the end of input with unclosed scopes
        state.is_finished()
            || error.contains("unexpected end")
            || error.contains("unclosed")
            || (state.remaining().trim().is_empty() && !state.stack.is_empty())
    }

    fn repair(&self, state: &mut ParseState, _error: &str) -> Result<(), FuzzyJsonError> {
        // Close all remaining scopes aggressively
        self.close_all_scopes(state)
    }
}

impl TruncationRepairStrategy {
    fn close_all_scopes(&self, state: &mut ParseState) -> Result<(), FuzzyJsonError> {
        // Track what we need to close
        let mut needs_closing = Vec::new();

        // Analyze the current state to determine what needs closing
        for context in state.stack.iter().rev() {
            match context {
                JsonContext::Object => needs_closing.push('}'),
                JsonContext::Array => needs_closing.push(']'),
                JsonContext::DoubleQuoteProperty |JsonContext::SingleQuoteProperty => {
                    // We might be in the middle of a property name or value
                    //
                    if state.output.chars().last() != Some('"')
                        && state.output.matches('"').count() % 2 != 0
                    {
                        println!("maybe the root cause @ 805");
                        needs_closing.push('"'); // Close unclosed string
                    }
                    // needs_closing.push('"'); // Close any unclosed string
                    needs_closing.push(':'); // set 0/empty
                    needs_closing.push('0'); // set 0/empty
                    // needs_closing.push('}'); // Close the object
                }
                JsonContext::Colon => {
                    needs_closing.push('0'); // set 0/empty
                }
                JsonContext::DoubleQuoteValue => {
                    // We might be in the middle of a value
                    if state.output.chars().last() == Some('"')
                        && state.output.matches('"').count() % 2 != 0
                    {
                        needs_closing.push('"'); // Close unclosed string
                    }
                }
                _ => {} // Root context doesn't need closing
            }
        }

        // Special case: if we're in the middle of a string
        if self.is_in_unclosed_string(&state.output) {
            // we can make this one redudant [todo:]
            needs_closing.insert(0, '"');
        }

        // Remove trailing comma if present
        if state.output.trim_end().ends_with(',') {
            let trimmed = state.output.trim_end();
            state.output = trimmed[..trimmed.len() - 1].to_string();
        }

        // Apply all closings
        for &closing_char in &needs_closing {
            state.output.push(closing_char);
        }

        Ok(())
    }

    fn is_in_unclosed_string(&self, output: &str) -> bool {
        let mut in_string = false;
        let mut escape_next = false;
        let mut quote_char = '"';

        for ch in output.chars() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match ch {
                '\\' if in_string => escape_next = true,
                '"' | '\'' if !in_string => {
                    in_string = true;
                    quote_char = ch;
                }
                c if in_string && c == quote_char => in_string = false,
                _ => {}
            }
        }

        in_string
    }
}

// Strategy for detecting and fixing incomplete property assignments
#[derive(Debug)]
pub struct IncompletePropertyStrategy;

impl RepairStrategy for IncompletePropertyStrategy {
    fn name(&self) -> &'static str {
        "incomplete_property"
    }
    fn priority(&self) -> u8 {
        85
    }

    fn can_repair(&self, state: &ParseState, _error: &str) -> bool {
        let output = state.output.trim_end();
        // Detect patterns like: "key": or "key":
        output.ends_with(':')
            || (output.ends_with('"') && state.remaining().trim().starts_with(':'))
    }

    fn repair(&self, state: &mut ParseState, _error: &str) -> Result<(), FuzzyJsonError> {
        let output = state.output.trim_end();

        if output.ends_with(':') {
            // Add a null value for incomplete property
            state.output.push_str(" null");
        } else if output.ends_with('"') && state.remaining().trim().starts_with(':') {
            // Complete the property assignment
            state.output.push_str(": null");
            // Skip the colon in remaining input
            while let Some(ch) = state.current_char() {
                if ch == ':' {
                    state.advance(1);
                    break;
                }
                if !ch.is_whitespace() {
                    break;
                }
                state.advance(1);
            }
        }

        Ok(())
    }
}

// Strategy for handling incomplete array elements
#[derive(Debug)]
pub struct IncompleteArrayStrategy;

impl RepairStrategy for IncompleteArrayStrategy {
    fn name(&self) -> &'static str {
        "incomplete_array"
    }
    fn priority(&self) -> u8 {
        80
    }

    fn can_repair(&self, state: &ParseState, _error: &str) -> bool {
        state.current_context() == &JsonContext::Array
            && state.output.trim_end().ends_with(',')
            && state.remaining().trim().is_empty()
    }

    fn repair(&self, state: &mut ParseState, _error: &str) -> Result<(), FuzzyJsonError> {
        // Remove trailing comma and close array
        let trimmed = state.output.trim_end();
        if trimmed.ends_with(',') {
            state.output = trimmed[..trimmed.len() - 1].to_string();
        }
        state.output.push(']');
        Ok(())
    }
}

// State Handlers
#[derive(Debug)]
pub struct WhitespaceHandler;

impl StateHandler for WhitespaceHandler {
    fn can_handle(&self, state: &ParseState) -> bool {
        state.current_char().map_or(false, |c| c.is_whitespace())
            || state.remaining().starts_with("\\n")
    }

    fn handle(&self, state: &mut ParseState) -> Result<bool, FuzzyJsonError> {
        while state.current_char().map_or(false, |a| a.is_whitespace())
            || state.remaining().starts_with("\\n")
        {
            // state.output.push(ch);
            if state.remaining().starts_with("\\n") {
                state.advance(2);
            } else {
                state.advance(1);
            }
        }
        Ok(true)
    }
}
#[derive(Debug)]
pub struct CommaHandler;

impl StateHandler for CommaHandler {
    fn can_handle(&self, state: &ParseState) -> bool {
        let remaining = state.remaining();
        /*
        println!(
            "start with [comma handler] | Current Char: {:?}: {:?}",
            state.current_char(),
            remaining.chars().nth(0)
        );*/
        remaining.starts_with(",")
    }

    fn handle(&self, state: &mut ParseState) -> Result<bool, FuzzyJsonError> {
        let remaining = state.remaining();

        if remaining.starts_with(",") {
            state.advance(1);

            let mut remaining = state.remaining();

            while remaining.starts_with("\\n")
                || state.current_char().map(|a| a.is_whitespace()) == Some(true)
            {
                if remaining.starts_with("\\n") {
                    state.advance(2);
                } else {
                    state.advance(1);
                }
                remaining = state.remaining();
            }
            
            // not an idiomatic way from first look, ideally it should have returned at this point
            // so that object handler could have taken over
            // but this is to handle a space case where comma is followed by closing curly brace,
            // as per json the stray comma is a syntax error
            if state.current_char() == Some('}') {
                state.output.push('}');
                state.advance(1);
                state.pop_context();
                return Ok(true);
            }
            state.output.push_str(",");
        }

        Ok(true)
    }
}
#[derive(Debug)]
pub struct ColonHandler;

impl StateHandler for ColonHandler {
    fn can_handle(&self, state: &ParseState) -> bool {
        let remaining = state.remaining();
        remaining.starts_with(":")
    }

    fn handle(&self, state: &mut ParseState) -> Result<bool, FuzzyJsonError> {
        // there should be a colon state as well
        // for the cases when json stopped at colon itself


        // println!("\n COLON: \n Remaining at colon handler check: {} | Context: {:?}", state.remaining(), state.current_context());
        if state.is_prop() {
            state.pop_context();
            state.push_context(JsonContext::Colon);
        }

        let remaining = state.remaining();
        if remaining.starts_with(":") {
            state.output.push(':');
            state.advance(1);
        }
        while state.current_char().map_or(false, |a| a.is_whitespace())
            || state.remaining().starts_with("\\n")
        {
            if state.remaining().starts_with("\\n") {
                state.advance(2);
            } else {
                state.advance(1);
            }
        }
        /*
        // not a right approach to add repair code in json handler
        // should be moved to repair strategies
        if state.current_char() == Some('}') {
            state.output.push_str("null");
            // state.advance(1);
            state.pop_context(); // colon context popped
        }*/

        Ok(true)
    }
}

#[derive(Debug)]
pub struct LiteralHandler;

impl StateHandler for LiteralHandler {
    fn can_handle(&self, state: &ParseState) -> bool {
        let remaining = state.remaining().trim();
        // println!("\n \n Remaining at literal handler check: {} | Context: {:?}", remaining, state.current_context());
        (state.current_context() == &JsonContext::Array
            || state.current_context() == &JsonContext::Colon
            || state.current_context().is_key())
            && (remaining.starts_with("true")
                || remaining.starts_with("false")
                || remaining.starts_with("null")
                || remaining.starts_with("undefined"))
    }

    fn handle(&self, state: &mut ParseState) -> Result<bool, FuzzyJsonError> {
        let remaining = state.remaining();

        if remaining.starts_with("true") {
            state.output.push_str("true");
            state.advance(4);
        } else if remaining.starts_with("false") {
            state.output.push_str("false");
            state.advance(5);
        } else if remaining.starts_with("null") {
            state.output.push_str("null");
            state.advance(4);
        }
        else if remaining.starts_with("undefined") {
            state.output.push_str("null");
            state.advance(9);
        }
        if state.current_context() != &JsonContext::Array {
            state.pop_context(); // if not array it would be a property or colon // what about
            // cases where literal appeared right after object
        }

        Ok(true)
    }
}

const VALID_KEY_FIRST_CHARS: [char; 27] = [
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', '_',
];
const VALID_KEY_REST_OF_CHARS: [char; 10] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];

#[derive(Debug)]
pub struct NoQuotesKeyHandler;

impl StateHandler for NoQuotesKeyHandler {
    fn can_handle(&self, state: &ParseState) -> bool {
        (state.current_context() == &JsonContext::Object)
            && (state
                .current_char()
                .map(|c| VALID_KEY_FIRST_CHARS.contains(&c.to_ascii_lowercase()))
                == Some(true))
    }

    fn handle(&self, state: &mut ParseState) -> Result<bool, FuzzyJsonError> {
        /*
        println!(
            "The mother fucker no quote inttervened at: {:?}  \n| {}",
            state.output,
            state.remaining()
        );*/
        state.push_context(JsonContext::DoubleQuoteProperty);
        state.output.push('"');

        while let Some(ch) = state.current_char() {
            if VALID_KEY_FIRST_CHARS.contains(&ch.to_ascii_lowercase())
                || VALID_KEY_REST_OF_CHARS.contains(&ch)
            {
                state.output.push(ch);
                state.advance(1);
            } else {
                state.output.push('"');
                break;
            }
        }

        Ok(true)
    }
}

#[derive(Debug)]
pub struct StringHandler;

impl StateHandler for StringHandler {
    fn can_handle(&self, state: &ParseState) -> bool {
        (state.is_sq_key_or_value() && state.current_char() == Some('\''))
            || (state.is_dq_key_or_value() && state.current_char() == Some('"'))
            || (!state.is_key_or_value()
                && (state.current_char() == Some('"') || state.current_char() == Some('\'')))
    }

    fn handle(&self, state: &mut ParseState) -> Result<bool, FuzzyJsonError> {
        /*
        println!(

            "{:?} | At the beginning of string handler: {}  | Output so far: {}",
            state.current_context(),
            state.remaining(),
            state.output
        );*/
        let boundary_char = state.current_char().unwrap(); // because this would be
        // called only if there
        // exists a current char

        state.output.push('"');
        state.advance(1);

        if state.current_context() == &JsonContext::Colon {
            state.pop_context();
            state.push_context(if boundary_char == '"' {
                JsonContext::DoubleQuoteValue
            } else {
                JsonContext::SingleQuoteValue
            });
        } else if state.is_prop() {
            // what if the colon is already there in json and it could be the next char itself
            //
            /*
            println!(
                "the fuck is going on here with this much remaining(something definitely seems wrong here): {}  | Output so far: {}",
                state.remaining(),
                state.output
            );*/
            // state.output.push(':');
            /*
            state.pop_context();
            state.push_context(if boundary_char == '"' {
                JsonContext::DoubleQuoteValue
            } else {
                JsonContext::SingleQuoteValue
            });*/
        } else if state.current_context() == &JsonContext::Array {
            state.push_context(if boundary_char == '"' {
                JsonContext::DoubleQuoteValue
            } else {
                JsonContext::SingleQuoteValue
            });
        } else {
            state.push_context(if boundary_char == '"' {
                JsonContext::DoubleQuoteProperty
            } else {
                JsonContext::SingleQuoteProperty
            });
        }

        while let Some(ch) = state.current_char() {
            if ch == boundary_char {
                state.output.push('"');
                state.advance(1);
                /*
                println!(
                    "stopped string-handler at {:?} | Remaning: {:?} | Current: {:?}",
                    state.position,
                    state.remaining().chars().nth(0),
                    state.current_char()
                );*/
                if state.is_value() {
                    state.pop_context();
                }
                break;
            }

            if ch == '\\' {
                state.output.push('\\');
                state.advance(1);
                if let Some(escaped) = state.current_char() {
                    state.output.push(escaped);
                    state.advance(1);
                }
            } else {
                state.output.push(ch);
                state.advance(1);
            }
        }

        Ok(true)
    }
}

#[derive(Debug)]
pub struct NumberHandler;

impl StateHandler for NumberHandler {
    fn can_handle(&self, state: &ParseState) -> bool {
        state
            .current_char()
            .map_or(false, |c| c.is_ascii_digit() || c == '-')
    }

    fn handle(&self, state: &mut ParseState) -> Result<bool, FuzzyJsonError> {
        if state.current_context() == &JsonContext::Colon {
            state.pop_context();
            state.push_context(JsonContext::DoubleQuoteValue);
        } else if state.current_context() == &JsonContext::DoubleQuoteProperty {
            state.pop_context();
            state.push_context(JsonContext::DoubleQuoteValue);
            state.output.push(':');
        } else if state.current_context() == &JsonContext::Array {
            state.push_context(JsonContext::DoubleQuoteValue);
        } else {
            state.push_context(JsonContext::DoubleQuoteProperty);
            state.output.push('"');
        }

        while let Some(ch) = state.current_char() {
            if ch.is_ascii_digit() || ch == '-' || ch == '+' || ch == '.' || ch == 'e' || ch == 'E'
            {
                state.output.push(ch);
                state.advance(1);
            } else {
                break;
            }
        }

        if state.current_context() == &JsonContext::DoubleQuoteValue {
            state.pop_context();
        } else if state.current_context() == &JsonContext::DoubleQuoteProperty
            && state
                .current_char()
                .map_or(true, |c| c.is_whitespace() || c == ':' || c == '}')
        {
            state.output.push('"');
        }
        Ok(true)
    }
}

#[derive(Debug)]
pub struct ObjectHandler;

impl StateHandler for ObjectHandler {
    fn can_handle(&self, state: &ParseState) -> bool {
        state.current_char() == Some('{')
            || (state.current_context() != &JsonContext::Root && state.current_char() == Some('}'))
    }

    fn handle(&self, state: &mut ParseState) -> Result<bool, FuzzyJsonError> {
        if state.current_context() == &JsonContext::Colon {
            state.pop_context();
        }
        if let Some(ch) = state.current_char() {
            if ch == '{' {
                state.output.push('{');
                state.push_context(JsonContext::Object);
                state.advance(1);
            } else if ch == '}' {
                state.output.push('}');
                state.pop_context();
                state.advance(1);
            }
        }
        Ok(true)
    }
}

#[derive(Debug)]
pub struct ArrayHandler;

impl StateHandler for ArrayHandler {
    fn can_handle(&self, state: &ParseState) -> bool {
        state.current_char() == Some('[') || state.current_char() == Some(']')
    }

    fn handle(&self, state: &mut ParseState) -> Result<bool, FuzzyJsonError> {
        if state.current_context() == &JsonContext::Colon {
            state.pop_context();
        }
        if let Some(ch) = state.current_char() {
            if ch == '[' {
                state.output.push('[');
                state.push_context(JsonContext::Array);
                state.advance(1);
            } else if ch == ']' {
                state.output.push(']');
                state.pop_context();
                state.advance(1);
            }
        }
        Ok(true)
    }
}

// Builder pattern for easy configuration
pub struct FuzzyJsonParserBuilder {
    options: ParserOptions,
    custom_strategies: Vec<Box<dyn RepairStrategy>>,
    custom_handlers: Vec<Box<dyn StateHandler>>,
}

impl FuzzyJsonParserBuilder {
    pub fn new() -> Self {
        Self {
            options: ParserOptions::default(),
            custom_strategies: Vec::new(),
            custom_handlers: Vec::new(),
        }
    }

    pub fn with_trailing_commas(mut self, allow: bool) -> Self {
        self.options.allow_trailing_commas = allow;
        self
    }

    pub fn with_single_quotes(mut self, allow: bool) -> Self {
        self.options.allow_single_quotes = allow;
        self
    }

    pub fn with_comments(mut self, allow: bool) -> Self {
        self.options.allow_comments = allow;
        self
    }

    pub fn with_unquoted_keys(mut self, allow: bool) -> Self {
        self.options.allow_unquoted_keys = allow;
        self
    }

    pub fn strict_mode(mut self, strict: bool) -> Self {
        self.options.strict_mode = strict;
        self
    }

    pub fn max_repair_attempts(mut self, max: usize) -> Self {
        self.options.max_repair_attempts = max;
        self
    }

    pub fn aggressive_truncation_repair(mut self, enable: bool) -> Self {
        self.options.aggressive_truncation_repair = enable;
        self
    }

    pub fn add_strategy(mut self, strategy: Box<dyn RepairStrategy>) -> Self {
        self.custom_strategies.push(strategy);
        self
    }

    pub fn add_handler(mut self, handler: Box<dyn StateHandler>) -> Self {
        self.custom_handlers.push(handler);
        self
    }

    pub fn build(self) -> FuzzyJsonParser {
        let mut parser = FuzzyJsonParser::with_options(self.options);

        for strategy in self.custom_strategies {
            parser.register_strategy(strategy);
        }

        for handler in self.custom_handlers {
            parser.register_handler(handler);
        }

        parser
    }
}

impl Default for FuzzyJsonParserBuilder {
    fn default() -> Self {
        Self::new()
    }
}
