use std::io::{Cursor, Seek, SeekFrom};

use mach_object::{OFile, MachCommand, LoadCommand, SymbolIter, Symbol};

#[derive(Debug)]
pub enum OfileErr {
    UnknownFormat,
    NoCode,
    Invalid
}

#[derive(Debug)]
pub enum CodeResult<'a> {
    UnknownBlock(&'a [u8], u64),
    Functions(Vec<(Option<String>, &'a [u8], u64)>)
}

pub enum MachoArch {
    X8664,
    Arm64
}

pub fn code_from(buf: &[u8]) -> Result<(CodeResult, Option<MachoArch>), OfileErr> {
    let mut cursor = Cursor::new(buf);

    let mut code = None;
    let mut symbols = None;

    fn extract_header_commands(ofile: OFile) -> Result<(mach_object::MachHeader, Vec<mach_object::MachCommand>), OfileErr> {
        match ofile {
            OFile::MachFile { header, commands } => Ok((header, commands)),
            OFile::FatFile { files, .. } => {
                if files.is_empty() {
                    return Err(OfileErr::NoCode)
                }

                // Prioritise arm64 for now, since we support that
                let mut last = None;
                for (arch, file) in files {
                    if arch.cputype == mach_object::CPU_TYPE_ARM64 {
                        return extract_header_commands(file);
                    }
                    last = Some(file);
                }
    
                extract_header_commands(last.unwrap())
            },
            _ => todo!()
        }
    }

    let (header, commands) = match OFile::parse(&mut cursor) {
        Ok(x) => extract_header_commands(x)?,
        Err(mach_object::MachError::UnknownMagic(_)) => return Err(OfileErr::UnknownFormat),
        Err(err) => {
            eprintln!("mach_object err: {err}");
            return Err(OfileErr::Invalid)
        }
    };

    let arch = match header.cputype {
        mach_object::CPU_TYPE_ARM64 => Some(MachoArch::Arm64),
        mach_object::CPU_TYPE_X86_64 => Some(MachoArch::X8664),
        _ => None
    };

    for MachCommand(cmd, ..) in &commands {
        if let &LoadCommand::Segment64 { segname, sections, .. } = &cmd {
            if segname != "__TEXT" {
                continue
            }

            for section in sections {
                if section.sectname != "__text" {
                    continue
                }

                code = Some((
                    section.addr,
                    section.offset as usize..section.offset as usize + section.size
                ));
            }
        }

        if let &LoadCommand::SymTab { symoff, nsyms, stroff, strsize } = &cmd {
            let sections = commands.iter().filter_map(|cmd| match cmd {
                MachCommand(LoadCommand::Segment { sections, .. }, ..)
                | MachCommand(LoadCommand::Segment64 { sections, .. }, ..) => Some(sections),
                _ => None,
            })
            .flat_map(|sections| sections.clone())
            .collect::<Vec<_>>();

            if cursor.seek(SeekFrom::Start(u64::from(*symoff))).is_ok() {
                let mut cur = cursor.clone();
                let iter = SymbolIter::new(
                    &mut cur,
                    sections,
                    *nsyms,
                    *stroff,
                    *strsize,
                    header.is_bigend(),
                    header.is_64bit(),
                );
                symbols = Some(iter.filter_map(|x| match x {
                    Symbol::Defined { name, section: Some(section), entry, .. } if section.sectname == "__text" => Some({
                        (name.map(str::to_string), entry)
                    }),
                    _ => None
                }).collect::<Vec<_>>());
            }
        }
    }

    let Some((code_vaddr, code)) = code else {
        return Err(OfileErr::NoCode)
    };

    let code = &buf[code];

    let Some(mut syms) = symbols else {
        return Ok((CodeResult::UnknownBlock(code, code_vaddr as u64), arch))
    };

    syms.sort_by_key(|(_, x)| *x);
    syms.push((None, code_vaddr + code.len()));

    let mut functions = Vec::new();
    for ((name, start), (_, end)) in syms.iter().zip(syms.iter().skip(1)) {
        if *start >= code_vaddr && *start <= code_vaddr + code.len() {
            functions.push((name.clone(), &code[*start - code_vaddr..*end - code_vaddr], *start as u64));
        }
    }

    Ok((CodeResult::Functions(functions), arch))
}
