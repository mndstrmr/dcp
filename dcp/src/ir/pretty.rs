use std::fmt::{Display, Write};

use crate::{mir, expr, Module, FunctionDecl};

pub struct PrettyPrintContext<'a> {
    indent: usize,
    module: Option<&'a Module>
}

impl<'a> PrettyPrintContext<'a> {
    pub fn new(module: &'a Module) -> PrettyPrintContext<'a> {
        PrettyPrintContext {
            indent: 0,
            module: Some(module)
        }
    }

    pub fn new_empty() -> PrettyPrintContext<'a> {
        PrettyPrintContext {
            indent: 0,
            module: None
        }
    }

    pub fn func(&self, func: expr::FuncId) -> Option<&'a FunctionDecl> {
        match &self.module {
            Some(module) => module.find_decl(func),
            None => None
        }
    }

    pub fn push_indent(&mut self) {
        self.indent += 1;
    }

    pub fn pop_indent(&mut self) {
        self.indent -= 1;
    }

    pub fn newline(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.write_char('\n')?;
        for _ in 0..self.indent {
            fmt.write_str("    ")?;
        }
        Ok(())
    }
}


pub struct PrettyPrinter<'a> {
    func: &'a mir::MirFunc,
    module: &'a Module
}

impl<'a> PrettyPrinter<'a> {
    pub fn new(func: &'a mir::MirFunc, module: &'a Module) -> PrettyPrinter<'a> {
        PrettyPrinter { func, module }
    }
}

impl<'a> Display for PrettyPrinter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ctx = PrettyPrintContext::new(self.module);
        self.func.fmt_named_with_context(f, &mut ctx)
    }
}
