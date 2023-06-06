use wasmparser::Payload;

pub enum WasmDecodeError {
    InvalidFormat,
    Invalid
}

// pub type Module<'a> = Vec<wasmparser::Payload<'a>>; 

pub struct Module<'a> {
    functions: Vec<wasmparser::FunctionBody<'a>>,
    types: Vec<wasmparser::FuncType>
}

impl<'a> Module<'a> {
    pub fn functions(&self) -> &[wasmparser::FunctionBody<'a>] {
        &self.functions
    }

    pub fn types(&self) -> &[wasmparser::FuncType] {
        &self.types
    }
}

pub fn module_from(buf: &[u8]) -> Result<Module, WasmDecodeError> {
    if !buf.starts_with(b"\0asm") {
        return Err(WasmDecodeError::InvalidFormat)
    }

    let mut res = Module { functions: Vec::new(), types: Vec::new() };

    for payload in wasmparser::Parser::new(0).parse_all(&buf) {
        match payload {
            Ok(Payload::CodeSectionEntry(body)) => res.functions.push(body),
            Ok(Payload::TypeSection(reader)) => {
                for ty in reader {
                    match ty {
                        Ok(wasmparser::Type::Func(func)) => res.types.push(func),
                        Ok(_) => {},
                        Err(err) => {
                            eprintln!("wasmparser err: {err}");
                            return Err(WasmDecodeError::Invalid)
                        }
                    }
                }
            },
            Ok(_) => (),
            Err(err) => {
                eprintln!("wasmparser err: {err}");
                return Err(WasmDecodeError::Invalid)
            }
        }
    }

    Ok(res)
}
