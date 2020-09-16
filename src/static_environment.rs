use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct StaticEnvironment {
    variables: HashSet<String>,
    stack: Vec<HashSet<String>>,
}

const FORWARD_ARGS: &'static str = "FORWARD_ARGS";

impl StaticEnvironment {
    pub fn new() -> Self {
        Self { variables: HashSet::new(), stack: vec![] }
    }

    pub fn reset(&mut self) {
        self.variables.clear();
        self.stack.clear();
    }

    pub fn extend_static(&mut self) {
        let mut variables: HashSet<String> = HashSet::new();
        std::mem::swap(&mut variables, &mut self.variables);
        self.stack.push(variables);
    }

    pub fn extend_dynamic(&mut self) {
        self.stack.push(self.variables.clone());
    }

    pub fn unextend(&mut self) {
        self.variables = self.stack.pop().unwrap();
    }

    pub fn declare(&mut self, name: &str) {
        self.variables.insert(name.to_owned());
    }

    pub fn is_declared(&self, name: &str) -> bool {
        self.variables.get(name).is_some()
    }

    pub fn declare_forward_args(&mut self) {
        self.declare(FORWARD_ARGS.into());
    }

    pub fn declared_forward_args(&self) -> bool {
        self.is_declared(FORWARD_ARGS.into())
    }
}
