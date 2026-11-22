//! Instructions, the counterpart of pykwasm's `binary/instructions.py`.
//!
//! Every instruction is wrapped in `aInstrWithPos(instr, offset, length)`: its offset in the
//! file and its length in bytes; for `block`, `loop` and `if`, up to and including their `end`.
//! The instructions pykwasm does not support are rejected here too.

use kore::symbols::Term;
use wasmparser::{
    BlockType, HeapType, MemArg, MemArgTarget, Operator, OperatorsReader, RefType, ValType,
};

use crate::Error;
use crate::ast::{Ast, R};

pub(crate) fn valtype(ast: &Ast, t: ValType, offset: u64) -> R<Term> {
    match t {
        ValType::I32 => ast.c("i32"),
        ValType::I64 => ast.c("i64"),
        ValType::F32 => ast.c("f32"),
        ValType::F64 => ast.c("f64"),
        ValType::Ref(rt) => reftype(ast, rt, offset),
        ValType::V128 => Err(Error::unsupported(offset, "the v128 value type")),
    }
}

pub(crate) fn reftype(ast: &Ast, rt: RefType, offset: u64) -> R<Term> {
    if rt == RefType::FUNCREF {
        ast.c("funcref")
    } else if rt == RefType::EXTERNREF {
        ast.c("externref")
    } else {
        Err(Error::unsupported(offset, format!("reference type {rt}")))
    }
}

fn blocktype(ast: &Ast, bt: BlockType, offset: u64) -> R<Term> {
    match bt {
        BlockType::Empty => ast.vec_type(vec![]),
        BlockType::Type(t) => ast.vec_type(vec![valtype(ast, t, offset)?]),
        BlockType::FuncType(_) => Err(Error::unsupported(
            offset,
            "block types given by a type index",
        )),
    }
}

enum Kind {
    Block,
    Loop,
    /// The `then` branch, once `else` has been seen.
    If(Option<Vec<Term>>),
}

/// A structured instruction whose body is being read.
struct Frame {
    kind: Kind,
    blocktype: Term,
    offset: u64,
    /// The instructions before it, at its own level.
    outer: Vec<Term>,
}

/// An expression: instructions up to the `end` that closes it, which is consumed. Function
/// bodies, and the constant expressions of globals and of element and data segments.
pub(crate) fn expr(ast: &Ast, mut ops: OperatorsReader) -> R<Vec<Term>> {
    let mut stack: Vec<Frame> = Vec::new();
    let mut current: Vec<Term> = Vec::new();
    loop {
        let (op, offset) = ops.read_with_offset()?;
        let after = ops.original_position();
        let mut open = |kind, bt| -> R<()> {
            let frame = Frame {
                kind,
                blocktype: blocktype(ast, bt, offset)?,
                offset,
                outer: std::mem::take(&mut current),
            };
            stack.push(frame);
            Ok(())
        };
        match op {
            Operator::Block { blockty } => open(Kind::Block, blockty)?,
            Operator::Loop { blockty } => open(Kind::Loop, blockty)?,
            Operator::If { blockty } => open(Kind::If(None), blockty)?,
            Operator::Else => match stack.last_mut() {
                Some(Frame {
                    kind: Kind::If(then @ None),
                    ..
                }) => *then = Some(std::mem::take(&mut current)),
                _ => return Err(Error::unsupported(offset, "`else` outside `if`")),
            },
            Operator::End => {
                let Some(frame) = stack.pop() else {
                    if !ops.eof() {
                        return Err(Error::unsupported(
                            after,
                            "bytes after the end of an expression",
                        ));
                    }
                    return Ok(current);
                };
                let body = std::mem::replace(&mut current, frame.outer);
                let instr = match frame.kind {
                    Kind::Block => ast.app("aBlock", vec![frame.blocktype, ast.instrs(body)?])?,
                    Kind::Loop => ast.app("aLoop", vec![frame.blocktype, ast.instrs(body)?])?,
                    Kind::If(then) => {
                        let (then, els) = match then {
                            Some(then) => (then, body),
                            None => (body, Vec::new()),
                        };
                        ast.app(
                            "aIf",
                            vec![frame.blocktype, ast.instrs(then)?, ast.instrs(els)?],
                        )?
                    }
                };
                current.push(ast.instr_with_pos(instr, frame.offset, after - frame.offset)?);
            }
            op => {
                let instr = plain(ast, op, offset)?;
                current.push(ast.instr_with_pos(instr, offset, after - offset)?);
            }
        }
    }
}

/// The memory index must be 0: the semantics models a single memory.
fn memidx(index: u32, offset: u64) -> R<()> {
    if index == 0 {
        Ok(())
    } else {
        Err(Error::unsupported(
            offset,
            format!("memory index {index} (multi-memory)"),
        ))
    }
}

/// An instruction other than `block`, `loop`, `if`, `else` and `end`.
fn plain(ast: &Ast, op: Operator, offset: u64) -> R<Term> {
    use Operator as O;
    let int = |v: u64| ast.int(v);
    let target = |m: &MemArg| match m.target {
        MemArgTarget::Memory(i) => memidx(i, offset),
        MemArgTarget::Array(_) => Err(Error::unsupported(offset, "GC array accesses")),
    };
    let load = |ty: &str, op: &str, m: MemArg| -> R<Term> {
        target(&m)?;
        ast.app("aLoad", vec![ast.c(ty)?, ast.c(op)?, int(m.offset)])
    };
    let store = |ty: &str, op: &str, m: MemArg| -> R<Term> {
        target(&m)?;
        ast.app("aStore", vec![ast.c(ty)?, ast.c(op)?, int(m.offset)])
    };
    let idx = |label: &str, i: u32| ast.app(label, vec![int(i.into())]);
    let t = |label: &str, ty: &str, op: &str| ast.typed(label, ty, op);
    match op {
        // Parametric
        O::Unreachable => ast.c("aUnreachable"),
        O::Nop => ast.c("aNop"),
        O::Drop => ast.c("aDrop"),
        // The operand types of a typed `select` are not kept, as in pykwasm.
        O::Select | O::TypedSelect { .. } | O::TypedSelectMulti { .. } => ast.c("aSelect"),

        // Control
        O::Br { relative_depth } => idx("aBr", relative_depth),
        O::BrIf { relative_depth } => idx("aBr_if", relative_depth),
        O::BrTable { targets } => {
            let mut labels = targets.targets().collect::<Result<Vec<u32>, _>>()?;
            labels.push(targets.default());
            ast.app("aBr_table", vec![ast.ints(labels)?])
        }
        O::Return => ast.c("aReturn"),
        O::Call { function_index } => idx("aCall", function_index),
        O::CallIndirect {
            type_index,
            table_index,
        } => {
            let type_use = idx("aTypeUseIndex", type_index)?;
            ast.app("aCall_indirect", vec![int(table_index.into()), type_use])
        }

        // Variables
        O::LocalGet { local_index } => idx("aLocal.get", local_index),
        O::LocalSet { local_index } => idx("aLocal.set", local_index),
        O::LocalTee { local_index } => idx("aLocal.tee", local_index),
        O::GlobalGet { global_index } => idx("aGlobal.get", global_index),
        O::GlobalSet { global_index } => idx("aGlobal.set", global_index),

        // Tables
        O::TableGet { table } => idx("aTable.get", table),
        O::TableSet { table } => idx("aTable.set", table),
        O::TableInit { elem_index, table } => ast.app(
            "aTable.init",
            vec![int(table.into()), int(elem_index.into())],
        ),
        O::ElemDrop { elem_index } => idx("aElem.drop", elem_index),
        O::TableCopy {
            dst_table,
            src_table,
        } => ast.app(
            "aTable.copy",
            vec![int(dst_table.into()), int(src_table.into())],
        ),
        O::TableGrow { table } => idx("aTable.grow", table),
        O::TableSize { table } => idx("aTable.size", table),
        O::TableFill { table } => idx("aTable.fill", table),

        // Memory
        O::I32Load { memarg } => load("i32", "loadOpLoad", memarg),
        O::I64Load { memarg } => load("i64", "loadOpLoad", memarg),
        O::F32Load { memarg } => load("f32", "loadOpLoad", memarg),
        O::F64Load { memarg } => load("f64", "loadOpLoad", memarg),
        O::I32Load8S { memarg } => load("i32", "loadOpLoad8_s", memarg),
        O::I32Load8U { memarg } => load("i32", "loadOpLoad8_u", memarg),
        O::I32Load16S { memarg } => load("i32", "loadOpLoad16_s", memarg),
        O::I32Load16U { memarg } => load("i32", "loadOpLoad16_u", memarg),
        O::I64Load8S { memarg } => load("i64", "loadOpLoad8_s", memarg),
        O::I64Load8U { memarg } => load("i64", "loadOpLoad8_u", memarg),
        O::I64Load16S { memarg } => load("i64", "loadOpLoad16_s", memarg),
        O::I64Load16U { memarg } => load("i64", "loadOpLoad16_u", memarg),
        O::I64Load32S { memarg } => load("i64", "loadOpLoad32_s", memarg),
        O::I64Load32U { memarg } => load("i64", "loadOpLoad32_u", memarg),
        O::I32Store { memarg } => store("i32", "storeOpStore", memarg),
        O::I64Store { memarg } => store("i64", "storeOpStore", memarg),
        O::F32Store { memarg } => store("f32", "storeOpStore", memarg),
        O::F64Store { memarg } => store("f64", "storeOpStore", memarg),
        O::I32Store8 { memarg } => store("i32", "storeOpStore8", memarg),
        O::I32Store16 { memarg } => store("i32", "storeOpStore16", memarg),
        O::I64Store8 { memarg } => store("i64", "storeOpStore8", memarg),
        O::I64Store16 { memarg } => store("i64", "storeOpStore16", memarg),
        O::I64Store32 { memarg } => store("i64", "storeOpStore32", memarg),
        O::MemorySize { mem } => memidx(mem, offset).and_then(|()| ast.c("aSize")),
        O::MemoryGrow { mem } => memidx(mem, offset).and_then(|()| ast.c("aGrow")),
        O::MemoryFill { mem } => memidx(mem, offset).and_then(|()| ast.c("aFill")),
        O::MemoryCopy { dst_mem, src_mem } => {
            memidx(dst_mem, offset)?;
            memidx(src_mem, offset)?;
            ast.c("aCopy")
        }

        // References
        O::RefNull { hty } => {
            let heap = if hty == HeapType::FUNC {
                "func"
            } else if hty == HeapType::EXTERN {
                "extern"
            } else {
                return Err(Error::unsupported(offset, format!("heap type {hty:?}")));
            };
            ast.app("aRef.null", vec![ast.c(heap)?])
        }
        O::RefIsNull => ast.c("aRef.is_null"),
        O::RefFunc { function_index } => idx("aRef.func", function_index),

        // Constants. Integers are written unsigned, as pykwasm writes them.
        O::I32Const { value } => {
            ast.app("aIConst", vec![ast.c("i32")?, int((value as u32).into())])
        }
        O::I64Const { value } => ast.app("aIConst", vec![ast.c("i64")?, int(value as u64)]),
        O::F32Const { value } => ast.app(
            "aFConst",
            vec![
                ast.c("f32")?,
                ast.float(f32::from_bits(value.bits()).into()),
            ],
        ),
        O::F64Const { value } => ast.app(
            "aFConst",
            vec![ast.c("f64")?, ast.float(f64::from_bits(value.bits()))],
        ),

        // Tests and comparisons
        O::I32Eqz => t("aTestOp", "i32", "aEqz"),
        O::I64Eqz => t("aTestOp", "i64", "aEqz"),
        O::I32Eq => t("aIRelOp", "i32", "intEq"),
        O::I32Ne => t("aIRelOp", "i32", "intNe"),
        O::I32LtS => t("aIRelOp", "i32", "intLt_s"),
        O::I32LtU => t("aIRelOp", "i32", "intLt_u"),
        O::I32GtS => t("aIRelOp", "i32", "intGt_s"),
        O::I32GtU => t("aIRelOp", "i32", "intGt_u"),
        O::I32LeS => t("aIRelOp", "i32", "intLe_s"),
        O::I32LeU => t("aIRelOp", "i32", "intLe_u"),
        O::I32GeS => t("aIRelOp", "i32", "intGe_s"),
        O::I32GeU => t("aIRelOp", "i32", "intGe_u"),
        O::I64Eq => t("aIRelOp", "i64", "intEq"),
        O::I64Ne => t("aIRelOp", "i64", "intNe"),
        O::I64LtS => t("aIRelOp", "i64", "intLt_s"),
        O::I64LtU => t("aIRelOp", "i64", "intLt_u"),
        O::I64GtS => t("aIRelOp", "i64", "intGt_s"),
        O::I64GtU => t("aIRelOp", "i64", "intGt_u"),
        O::I64LeS => t("aIRelOp", "i64", "intLe_s"),
        O::I64LeU => t("aIRelOp", "i64", "intLe_u"),
        O::I64GeS => t("aIRelOp", "i64", "intGe_s"),
        O::I64GeU => t("aIRelOp", "i64", "intGe_u"),
        O::F32Eq => t("aFRelOp", "f32", "floatEq"),
        O::F32Ne => t("aFRelOp", "f32", "floatNe"),
        O::F32Lt => t("aFRelOp", "f32", "floatLt"),
        O::F32Gt => t("aFRelOp", "f32", "floatGt"),
        O::F32Le => t("aFRelOp", "f32", "floatLe"),
        O::F32Ge => t("aFRelOp", "f32", "floatGe"),
        O::F64Eq => t("aFRelOp", "f64", "floatEq"),
        O::F64Ne => t("aFRelOp", "f64", "floatNe"),
        O::F64Lt => t("aFRelOp", "f64", "floatLt"),
        O::F64Gt => t("aFRelOp", "f64", "floatGt"),
        O::F64Le => t("aFRelOp", "f64", "floatLe"),
        O::F64Ge => t("aFRelOp", "f64", "floatGe"),

        // Integer arithmetic
        O::I32Clz => t("aIUnOp", "i32", "aClz"),
        O::I32Ctz => t("aIUnOp", "i32", "aCtz"),
        O::I32Popcnt => t("aIUnOp", "i32", "aPopcnt"),
        O::I64Clz => t("aIUnOp", "i64", "aClz"),
        O::I64Ctz => t("aIUnOp", "i64", "aCtz"),
        O::I64Popcnt => t("aIUnOp", "i64", "aPopcnt"),
        O::I32Add => t("aIBinOp", "i32", "intAdd"),
        O::I32Sub => t("aIBinOp", "i32", "intSub"),
        O::I32Mul => t("aIBinOp", "i32", "intMul"),
        O::I32DivS => t("aIBinOp", "i32", "intDiv_s"),
        O::I32DivU => t("aIBinOp", "i32", "intDiv_u"),
        O::I32RemS => t("aIBinOp", "i32", "intRem_s"),
        O::I32RemU => t("aIBinOp", "i32", "intRem_u"),
        O::I32And => t("aIBinOp", "i32", "intAnd"),
        O::I32Or => t("aIBinOp", "i32", "intOr"),
        O::I32Xor => t("aIBinOp", "i32", "intXor"),
        O::I32Shl => t("aIBinOp", "i32", "intShl"),
        O::I32ShrS => t("aIBinOp", "i32", "intShr_s"),
        O::I32ShrU => t("aIBinOp", "i32", "intShr_u"),
        O::I32Rotl => t("aIBinOp", "i32", "intRotl"),
        O::I32Rotr => t("aIBinOp", "i32", "intRotr"),
        O::I64Add => t("aIBinOp", "i64", "intAdd"),
        O::I64Sub => t("aIBinOp", "i64", "intSub"),
        O::I64Mul => t("aIBinOp", "i64", "intMul"),
        O::I64DivS => t("aIBinOp", "i64", "intDiv_s"),
        O::I64DivU => t("aIBinOp", "i64", "intDiv_u"),
        O::I64RemS => t("aIBinOp", "i64", "intRem_s"),
        O::I64RemU => t("aIBinOp", "i64", "intRem_u"),
        O::I64And => t("aIBinOp", "i64", "intAnd"),
        O::I64Or => t("aIBinOp", "i64", "intOr"),
        O::I64Xor => t("aIBinOp", "i64", "intXor"),
        O::I64Shl => t("aIBinOp", "i64", "intShl"),
        O::I64ShrS => t("aIBinOp", "i64", "intShr_s"),
        O::I64ShrU => t("aIBinOp", "i64", "intShr_u"),
        O::I64Rotl => t("aIBinOp", "i64", "intRotl"),
        O::I64Rotr => t("aIBinOp", "i64", "intRotr"),

        // Float arithmetic
        O::F32Abs => t("aFUnOp", "f32", "aAbs"),
        O::F32Neg => t("aFUnOp", "f32", "aNeg"),
        O::F32Ceil => t("aFUnOp", "f32", "aCeil"),
        O::F32Floor => t("aFUnOp", "f32", "aFloor"),
        O::F32Trunc => t("aFUnOp", "f32", "aTrunc"),
        O::F32Nearest => t("aFUnOp", "f32", "aNearest"),
        O::F32Sqrt => t("aFUnOp", "f32", "aSqrt"),
        O::F64Abs => t("aFUnOp", "f64", "aAbs"),
        O::F64Neg => t("aFUnOp", "f64", "aNeg"),
        O::F64Ceil => t("aFUnOp", "f64", "aCeil"),
        O::F64Floor => t("aFUnOp", "f64", "aFloor"),
        O::F64Trunc => t("aFUnOp", "f64", "aTrunc"),
        O::F64Nearest => t("aFUnOp", "f64", "aNearest"),
        O::F64Sqrt => t("aFUnOp", "f64", "aSqrt"),
        O::F32Add => t("aFBinOp", "f32", "floatAdd"),
        O::F32Sub => t("aFBinOp", "f32", "floatSub"),
        O::F32Mul => t("aFBinOp", "f32", "floatMul"),
        O::F32Div => t("aFBinOp", "f32", "floatDiv"),
        O::F32Min => t("aFBinOp", "f32", "floatMin"),
        O::F32Max => t("aFBinOp", "f32", "floatMax"),
        O::F32Copysign => t("aFBinOp", "f32", "floatCopysign"),
        O::F64Add => t("aFBinOp", "f64", "floatAdd"),
        O::F64Sub => t("aFBinOp", "f64", "floatSub"),
        O::F64Mul => t("aFBinOp", "f64", "floatMul"),
        O::F64Div => t("aFBinOp", "f64", "floatDiv"),
        O::F64Min => t("aFBinOp", "f64", "floatMin"),
        O::F64Max => t("aFBinOp", "f64", "floatMax"),
        O::F64Copysign => t("aFBinOp", "f64", "floatCopysign"),

        // Conversions
        O::I32WrapI64 => t("aCvtOp", "i32", "aWrap_i64"),
        O::I32TruncF32S => t("aCvtOp", "i32", "aTrunc_f32_s"),
        O::I32TruncF32U => t("aCvtOp", "i32", "aTrunc_f32_u"),
        O::I32TruncF64S => t("aCvtOp", "i32", "aTrunc_f64_s"),
        O::I32TruncF64U => t("aCvtOp", "i32", "aTrunc_f64_u"),
        O::I64ExtendI32S => t("aCvtOp", "i64", "aExtend_i32_s"),
        O::I64ExtendI32U => t("aCvtOp", "i64", "aExtend_i32_u"),
        O::I64TruncF32S => t("aCvtOp", "i64", "aTrunc_f32_s"),
        O::I64TruncF32U => t("aCvtOp", "i64", "aTrunc_f32_u"),
        O::I64TruncF64S => t("aCvtOp", "i64", "aTrunc_f64_s"),
        O::I64TruncF64U => t("aCvtOp", "i64", "aTrunc_f64_u"),
        O::F32ConvertI32S => t("aCvtOp", "f32", "aConvert_i32_s"),
        O::F32ConvertI32U => t("aCvtOp", "f32", "aConvert_i32_u"),
        O::F32ConvertI64S => t("aCvtOp", "f32", "aConvert_i64_s"),
        O::F32ConvertI64U => t("aCvtOp", "f32", "aConvert_i64_u"),
        O::F32DemoteF64 => t("aCvtOp", "f32", "aDemote_f64"),
        O::F64ConvertI32S => t("aCvtOp", "f64", "aConvert_i32_s"),
        O::F64ConvertI32U => t("aCvtOp", "f64", "aConvert_i32_u"),
        O::F64ConvertI64S => t("aCvtOp", "f64", "aConvert_i64_s"),
        O::F64ConvertI64U => t("aCvtOp", "f64", "aConvert_i64_u"),
        O::F64PromoteF32 => t("aCvtOp", "f64", "aPromote_f32"),

        // Sign extension. pykwasm gives the three i64 forms the type i32 (`kwasm_ast.py`,
        // I64_EXTEND*_s), which makes them compute on 32 bits; here they get i64.
        O::I32Extend8S => t("aExtendS", "i32", "aExtend8_s"),
        O::I32Extend16S => t("aExtendS", "i32", "aExtend16_s"),
        O::I64Extend8S => t("aExtendS", "i64", "aExtend8_s"),
        O::I64Extend16S => t("aExtendS", "i64", "aExtend16_s"),
        O::I64Extend32S => t("aExtendS", "i64", "aExtend32_s"),

        // Not supported by the semantics (pykwasm rejects them too): reinterpretations,
        // saturating truncations, memory.init, data.drop, SIMD, atomics, exceptions, ...
        op => Err(Error::unsupported(offset, format!("instruction {op:?}"))),
    }
}
