//! Modules, the counterpart of pykwasm's `binary/module.py`.

use kore::symbols::Term;
use wasmparser::{
    CompositeInnerType, DataKind, ElementItems, ElementKind, Encoding, ExternalKind, Operator,
    Parser, Payload, TableInit, TypeRef,
};

use crate::Error;
use crate::ast::{Ast, R};
use crate::instructions::{expr, reftype, valtype};

/// The sections, as lists of definitions, in the order of the arguments of `aModuleDecl`.
#[derive(Default)]
struct Sections {
    types: Vec<Term>,
    /// Type indices of the functions, from the function section.
    func_types: Vec<u32>,
    /// Locals and bodies of the functions, from the code section.
    codes: Vec<(Vec<Term>, Vec<Term>)>,
    tables: Vec<Term>,
    mems: Vec<Term>,
    globals: Vec<Term>,
    elems: Vec<Term>,
    datas: Vec<Term>,
    start: Vec<Term>,
    imports: Vec<Term>,
    exports: Vec<Term>,
}

pub(crate) fn module(ast: &Ast, wasm: &[u8], filename: Option<&str>) -> R<Term> {
    let mut s = Sections::default();
    for payload in Parser::new(0).parse_all(wasm) {
        let payload = payload?;
        match payload {
            Payload::Version {
                encoding, range, ..
            } => {
                if encoding != Encoding::Module {
                    return Err(Error::unsupported(range.start, "components"));
                }
            }
            Payload::TypeSection(r) => {
                for group in r.into_iter_with_offsets() {
                    let (offset, group) = group?;
                    let types: Vec<_> = group.into_types().collect();
                    let [ty] = &types[..] else {
                        return Err(Error::unsupported(offset, "recursive types"));
                    };
                    if !ty.supertype_idxs.is_empty() {
                        return Err(Error::unsupported(offset, "subtypes"));
                    }
                    let CompositeInnerType::Func(f) = &ty.composite_type.inner else {
                        return Err(Error::unsupported(
                            offset,
                            "composite types other than functions",
                        ));
                    };
                    let params = f
                        .params()
                        .iter()
                        .map(|&t| valtype(ast, t, offset))
                        .collect::<R<_>>()?;
                    let results = f
                        .results()
                        .iter()
                        .map(|&t| valtype(ast, t, offset))
                        .collect::<R<_>>()?;
                    let func_type = ast.app(
                        "aFuncType",
                        vec![ast.vec_type(params)?, ast.vec_type(results)?],
                    )?;
                    s.types
                        .push(ast.app("aTypeDefn", vec![func_type, ast.empty_id()?])?);
                }
            }
            Payload::ImportSection(r) => {
                for import in r.into_imports_with_offsets() {
                    let (offset, import) = import?;
                    let desc = match import.ty {
                        TypeRef::Func(t) => {
                            ast.app("aFuncDesc", vec![ast.empty_id()?, ast.int(t)])?
                        }
                        TypeRef::Table(t) => {
                            if t.element_type != wasmparser::RefType::FUNCREF {
                                return Err(Error::unsupported(
                                    offset,
                                    "importing a table not of funcref",
                                ));
                            }
                            let limits = table_limits(ast, &t, offset)?;
                            ast.app("aTableDesc", vec![ast.empty_id()?, limits])?
                        }
                        TypeRef::Memory(m) => {
                            let limits = memory_limits(ast, &m, offset)?;
                            ast.app("aMemoryDesc", vec![ast.empty_id()?, limits])?
                        }
                        TypeRef::Global(g) => {
                            let global_type = global_type(ast, &g, offset)?;
                            ast.app("aGlobalDesc", vec![ast.empty_id()?, global_type])?
                        }
                        _ => {
                            return Err(Error::unsupported(
                                offset,
                                format!("import of {:?}", import.ty),
                            ));
                        }
                    };
                    let names = vec![
                        ast.wasm_string(import.module),
                        ast.wasm_string(import.name),
                        desc,
                    ];
                    s.imports.push(ast.app("aImportDefn", names)?);
                }
            }
            Payload::FunctionSection(r) => {
                for t in r {
                    s.func_types.push(t?);
                }
            }
            Payload::TableSection(r) => {
                for table in r.into_iter_with_offsets() {
                    let (offset, table) = table?;
                    if let TableInit::Expr(_) = table.init {
                        return Err(Error::unsupported(
                            offset,
                            "tables with an initializer expression",
                        ));
                    }
                    let limits = table_limits(ast, &table.ty, offset)?;
                    let rt = reftype(ast, table.ty.element_type, offset)?;
                    s.tables
                        .push(ast.app("aTableDefn", vec![limits, rt, ast.empty_id()?])?);
                }
            }
            Payload::MemorySection(r) => {
                for mem in r.into_iter_with_offsets() {
                    let (offset, mem) = mem?;
                    let limits = memory_limits(ast, &mem, offset)?;
                    s.mems
                        .push(ast.app("aMemoryDefn", vec![limits, ast.empty_id()?])?);
                }
            }
            Payload::GlobalSection(r) => {
                for global in r.into_iter_with_offsets() {
                    let (offset, global) = global?;
                    let ty = global_type(ast, &global.ty, offset)?;
                    let init = ast.instrs(expr(ast, global.init_expr.get_operators_reader())?)?;
                    s.globals
                        .push(ast.app("aGlobalDefn", vec![ty, init, ast.empty_id()?])?);
                }
            }
            Payload::ExportSection(r) => {
                for export in r.into_iter_with_offsets() {
                    let (offset, export) = export?;
                    let label = match export.kind {
                        ExternalKind::Func | ExternalKind::FuncExact => "aExternIdxFunc",
                        ExternalKind::Table => "aExternIdxTable",
                        ExternalKind::Memory => {
                            if export.index != 0 {
                                return Err(Error::unsupported(
                                    offset,
                                    "memory index other than 0",
                                ));
                            }
                            "aExternIdxMemory"
                        }
                        ExternalKind::Global => "aExternIdxGlobal",
                        ExternalKind::Tag => "aExternIdxTag",
                    };
                    let index = ast.app(label, vec![ast.int(export.index)])?;
                    s.exports
                        .push(ast.app("aExportDefn", vec![ast.wasm_string(export.name), index])?);
                }
            }
            Payload::StartSection { func, .. } => {
                s.start.push(ast.app("aStartDefn", vec![ast.int(func)])?);
            }
            Payload::ElementSection(r) => {
                for elem in r.into_iter_with_offsets() {
                    let (offset, elem) = elem?;
                    let mode = match elem.kind {
                        ElementKind::Passive => ast.c("aElemPassive")?,
                        ElementKind::Declared => ast.c("aElemDeclarative")?,
                        ElementKind::Active {
                            table_index,
                            offset_expr,
                        } => {
                            let table = ast.int(table_index.unwrap_or(0));
                            let offset_instrs =
                                ast.instrs(expr(ast, offset_expr.get_operators_reader())?)?;
                            ast.app("aElemActive", vec![table, offset_instrs])?
                        }
                    };
                    let (rt, init) = match elem.items {
                        ElementItems::Functions(r) => {
                            let indices = r
                                .into_iter()
                                .map(|i| i.map(Some))
                                .collect::<Result<Vec<_>, _>>()?;
                            (ast.c("funcref")?, indices)
                        }
                        ElementItems::Expressions(rt, r) => {
                            let mut init = Vec::new();
                            for e in r {
                                init.push(elem_init(e?.get_operators_reader(), offset)?);
                            }
                            (reftype(ast, rt, offset)?, init)
                        }
                    };
                    let refs = refs(ast, init)?;
                    s.elems
                        .push(ast.app("aElemDefn", vec![rt, refs, mode, ast.empty_id()?])?);
                }
            }
            Payload::DataCountSection { .. }
            | Payload::CustomSection(_)
            | Payload::CodeSectionStart { .. } => {}
            Payload::CodeSectionEntry(body) => {
                let mut locals = Vec::new();
                let mut reader = body.get_locals_reader()?;
                for _ in 0..reader.get_count() {
                    let offset = reader.original_position();
                    let (count, t) = reader.read()?;
                    let t = valtype(ast, t, offset)?;
                    locals.extend(std::iter::repeat_n(t, count as usize));
                }
                let code = expr(ast, body.get_operators_reader()?)?;
                s.codes.push((locals, code));
            }
            Payload::DataSection(r) => {
                for data in r.into_iter_with_offsets() {
                    let (offset, data) = data?;
                    let mode = match data.kind {
                        DataKind::Passive => ast.c("aDataModePassive")?,
                        DataKind::Active {
                            memory_index,
                            offset_expr,
                        } => {
                            if memory_index != 0 {
                                return Err(Error::unsupported(
                                    offset,
                                    "memory index other than 0",
                                ));
                            }
                            let offset_instrs =
                                ast.instrs(expr(ast, offset_expr.get_operators_reader())?)?;
                            ast.app(
                                "aDataModeActive",
                                vec![ast.int(memory_index), offset_instrs],
                            )?
                        }
                    };
                    s.datas
                        .push(ast.app("aDataDefn", vec![ast.bytes(data.data), mode])?);
                }
            }
            Payload::End(_) => break,
            other => {
                let offset = other.as_section().map_or(0, |(_, range)| range.start);
                return Err(Error::unsupported(
                    offset,
                    "a section the semantics does not support",
                ));
            }
        }
    }

    if s.func_types.len() != s.codes.len() {
        return Err(Error::unsupported(
            wasm.len(),
            format!(
                "{} functions but {} bodies",
                s.func_types.len(),
                s.codes.len()
            ),
        ));
    }
    let mut funcs = Vec::new();
    for (t, (locals, code)) in s.func_types.into_iter().zip(s.codes) {
        let meta = ast.app("funcMeta", vec![ast.empty_id()?, ast.c(".Map")?])?;
        funcs.push(ast.app(
            "aFuncDefn",
            vec![ast.int(t), ast.vec_type(locals)?, ast.instrs(code)?, meta],
        )?);
    }
    let filename = match filename {
        Some(f) => ast.string(f),
        None => ast.c(".String")?,
    };
    let meta = ast.app(
        "moduleMeta",
        vec![ast.empty_id()?, ast.c(".Map")?, filename],
    )?;
    let args = vec![
        ast.defns(s.types)?,
        ast.defns(funcs)?,
        ast.defns(s.tables)?,
        ast.defns(s.mems)?,
        ast.defns(s.globals)?,
        ast.defns(s.elems)?,
        ast.defns(s.datas)?,
        ast.defns(s.start)?,
        ast.defns(s.imports)?,
        ast.defns(s.exports)?,
        meta,
    ];
    ast.app("aModuleDecl", args)
}

/// An element initializer: a single `ref.func` (`Some`) or `ref.null` (`None`).
fn elem_init(mut ops: wasmparser::OperatorsReader, offset: u64) -> R<Option<u32>> {
    let first = ops.read()?;
    let end = ops.read()?;
    let value = match (first, end) {
        (Operator::RefFunc { function_index }, Operator::End) => Some(function_index),
        (Operator::RefNull { .. }, Operator::End) => None,
        _ => {
            return Err(Error::unsupported(
                offset,
                "element initializers other than ref.func and ref.null",
            ));
        }
    };
    Ok(value)
}

/// `_ListRef_` of `ListRefItem(RefVal(funcref, i))` or `ListRefItem(RefValNull(funcref))`.
fn refs(ast: &Ast, init: Vec<Option<u32>>) -> R<Term> {
    let items = init
        .into_iter()
        .map(|i| {
            let r = match i {
                Some(i) => ast.app("RefVal", vec![ast.c("funcref")?, ast.int(i)])?,
                None => ast.app("RefValNull", vec![ast.c("funcref")?])?,
            };
            ast.app("ListRefItem", vec![r])
        })
        .collect::<R<_>>()?;
    ast.list("_ListRef_", ".ListRef", items)
}

fn table_limits(ast: &Ast, t: &wasmparser::TableType, offset: u64) -> R<Term> {
    if t.table64 {
        return Err(Error::unsupported(offset, "64-bit tables (table64)"));
    }
    if t.shared {
        return Err(Error::unsupported(offset, "shared tables"));
    }
    ast.limits(t.initial, t.maximum)
}

fn memory_limits(ast: &Ast, m: &wasmparser::MemoryType, offset: u64) -> R<Term> {
    if m.memory64 {
        return Err(Error::unsupported(offset, "64-bit memories (memory64)"));
    }
    if m.shared {
        return Err(Error::unsupported(offset, "shared memories"));
    }
    if m.page_size_log2.is_some() {
        return Err(Error::unsupported(offset, "custom page sizes"));
    }
    ast.limits(m.initial, m.maximum)
}

fn global_type(ast: &Ast, g: &wasmparser::GlobalType, offset: u64) -> R<Term> {
    if g.shared {
        return Err(Error::unsupported(offset, "shared globals"));
    }
    let mutability = ast.c(if g.mutable { "mutVar" } else { "mutConst" })?;
    ast.app(
        "aGlobalType",
        vec![mutability, valtype(ast, g.content_type, offset)?],
    )
}
