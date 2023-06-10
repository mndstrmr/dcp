use wasmparser::Payload;

pub enum WasmDecodeError {
    InvalidFormat,
    Invalid
}

// pub type Module<'a> = Vec<wasmparser::Payload<'a>>; 

pub struct Module<'a> {
    functions: Vec<wasmparser::FunctionBody<'a>>,
    types: Vec<wasmparser::FuncType>,
    // import_count: usize
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
    let mut types = Vec::new();

    for payload in wasmparser::Parser::new(0).parse_all(&buf) {
        match payload {
            Ok(Payload::CodeSectionEntry(body)) => res.functions.push(body),
            Ok(Payload::TypeSection(reader)) => {
                for ty in reader {
                    match ty {
                        Ok(wasmparser::Type::Func(func)) => types.push(func),
                        Ok(_) => {},
                        Err(err) => {
                            eprintln!("wasmparser err: {err}");
                            return Err(WasmDecodeError::Invalid)
                        }
                    }
                }
            },
            Ok(Payload::ImportSection(reader)) => {
                for import in reader {
                    match import {
                        Ok(x) => match x.ty {
                            wasmparser::TypeRef::Func(func) => res.types.push(types[func as usize].clone()),
                            _ => {}
                        },
                        Err(err) => {
                            eprintln!("wasmparser err: {err}");
                            return Err(WasmDecodeError::Invalid)
                        }
                    }
                }
            }
            Ok(Payload::FunctionSection(reader)) => {
                for ty in reader {
                    match ty {
                        Ok(x) => res.types.push(types[x as usize].clone()),
                        Err(err) => {
                            eprintln!("wasmparser err: {err}");
                            return Err(WasmDecodeError::Invalid)
                        }
                    }
                }
            }
            Ok(_) => (),
            Err(err) => {
                eprintln!("wasmparser err: {err}");
                return Err(WasmDecodeError::Invalid)
            }
        }
    }

    // assert_eq!(res.types.len(), res.functions.len());

    Ok(res)
}
