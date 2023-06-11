use std::collections::HashMap;

use wasmparser::Payload;

pub enum WasmDecodeError {
    InvalidFormat,
    Invalid
}

pub struct Function<'a> {
    pub body: wasmparser::FunctionBody<'a>,
    pub name: Option<String>,
    pub idx: usize,
    pub exported: bool
}

pub struct Import {
    pub name: String,
    pub idx: usize
}

pub struct Module<'a> {
    functions: Vec<Function<'a>>,
    types: Vec<wasmparser::FuncType>,
    imports: Vec<Import>,
    raw_types: Vec<wasmparser::FuncType>,
}

impl<'a> Module<'a> {
    pub fn functions(&self) -> &[Function<'a>] {
        &self.functions
    }

    pub fn imports(&self) -> &[Import] {
        &self.imports
    }

    pub fn types(&self) -> &[wasmparser::FuncType] {
        &self.types
    }

    pub fn raw_types(&self) -> &[wasmparser::FuncType] {
        &self.raw_types
    }
}

pub fn module_from(buf: &[u8]) -> Result<Module, WasmDecodeError> {
    if !buf.starts_with(b"\0asm") {
        return Err(WasmDecodeError::InvalidFormat)
    }

    let mut res = Module {
        functions: Vec::new(),
        types: Vec::new(),
        imports: Vec::new(),
        raw_types: Vec::new()
    };
    let mut import_count = 0;
    let mut names = HashMap::new();

    for payload in wasmparser::Parser::new(0).parse_all(&buf) {
        match payload {
            Ok(Payload::CodeSectionEntry(body)) => res.functions.push(Function {
                body,
                name: names.get(&(res.functions.len() + import_count)).map(|x| str::to_string(*x)),
                idx: res.functions.len() + import_count,
                exported: names.contains_key(&(res.functions.len() + import_count))
            }),
            Ok(Payload::TypeSection(reader)) => {
                for ty in reader {
                    match ty {
                        Ok(wasmparser::Type::Func(func)) => res.raw_types.push(func),
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
                            wasmparser::TypeRef::Func(func) => {
                                res.types.push(res.raw_types[func as usize].clone());
                                res.imports.push(Import {
                                    idx: import_count,
                                    name: x.name.to_string()
                                });
                                import_count += 1;
                            },
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
                        Ok(x) => res.types.push(res.raw_types[x as usize].clone()),
                        Err(err) => {
                            eprintln!("wasmparser err: {err}");
                            return Err(WasmDecodeError::Invalid)
                        }
                    }
                }
            }
            Ok(Payload::ExportSection(reader)) => {
                for export in reader {
                    match export {
                        Ok(x) => match x.kind {
                            wasmparser::ExternalKind::Func => {
                                names.insert(x.index as usize, x.name);
                            },
                            _ => {}
                        },
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
