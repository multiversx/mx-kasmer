```k
module REPLACE-AT-LEMMAS-BASIC
    imports HANDLE-SPARSE-BYTES-LEMMAS-SYNTAX

    syntax SBSetFunction ::= replaceAt(value:Bytes)
endmodule

module REPLACE-AT-LEMMAS
    imports REPLACE-AT-LEMMAS-BASIC

    rule functionSparseBytesWellDefined(
            replaceAt(_Val:Bytes), Addr:Int, Width:Int
        )
        => 0 <=Int Addr andBool 0 <Int Width
    rule updateSparseBytesSize(
            replaceAt(Val:Bytes), SBSize:Int, Addr:Int, Width:Int
        )
        => maxInt(SBSize, Addr +Int Width)
        requires functionSparseBytesWellDefined(
            replaceAt(Val:Bytes), Addr, Width
        )
    rule startOffset(replaceAt(_:Bytes)) => 0
    rule functionCommutesAtStart(replaceAt(_:Bytes)) => true
    rule getReplacementSparseBytes(
            replaceAt(Value:Bytes), Addr:Int, Width:Int
        )
        => SBChunk(#bytes(Value))
        requires functionSparseBytesWellDefined(
                replaceAt(Value:Bytes), Addr, Width
            )
          andBool Width ==Int lengthBytes(Value)

    // Until wasm-semantics 0.1.106 the next two rules were part of the definition
    // of `replaceAt` and applied to any arguments. Since 0.1.157 the symbolic
    // backend only has them for fully concrete arguments, which leaves writes of
    // a symbolic value into a known chunk to the generic `updateSparseBytes`
    // lemmas, and those do not cover every case (a write that is not aligned
    // with the chunk, or runs past its end). They are restored here, ahead of
    // the conversion to `updateSparseBytes` below.
    rule replaceAt(SBChunk(#empty(N)) REST, S, Bs) => replaceAtZ(N, REST, S, Bs)
        requires 0 <=Int S andBool S <Int N
      [simplification(40)]
    rule replaceAt(SBChunk(#bytes(B)) REST, S, Bs) => replaceAtB(B, REST, S, Bs)
        requires 0 <=Int S andBool S <Int lengthBytes(B)
      [simplification(40)]

    rule replaceAt(SB:SparseBytes, Addr:Int, Value:Bytes)
        => updateSparseBytes(
              replaceAt(Value), SB, Addr, lengthBytes(Value)
          )
        requires functionSparseBytesWellDefined(
              replaceAt(Value), Addr, lengthBytes(Value)
          )
      [simplification, symbolic(SB)]
    rule replaceAt(SB:SparseBytes, Addr:Int, Value:Bytes)
        => updateSparseBytes(
              replaceAt(Value), SB, Addr, lengthBytes(Value)
          )
        requires functionSparseBytesWellDefined(
              replaceAt(Value), Addr, lengthBytes(Value)
          )
      [simplification, symbolic(Addr)]
    rule replaceAt(SB:SparseBytes, Addr:Int, Value:Bytes)
        => updateSparseBytes(
              replaceAt(Value), SB, Addr, lengthBytes(Value)
          )
        requires functionSparseBytesWellDefined(
              replaceAt(Value), Addr, lengthBytes(Value)
          )
      [simplification, symbolic(Value)]

    rule updateSparseBytes(
              replaceAt(Value), SB, Addr, Width
          )
        => replaceAt(SB, Addr, Value)
        requires functionSparseBytesWellDefined(
              replaceAt(Value), Addr, Width
          )
          andBool Width ==Int lengthBytes(Value)
      [simplification, concrete]

    rule updateSparseBytes(
              replaceAt(Value), SB, 0, Width
          )
        => SBChunk(#bytes(Value))
        requires functionSparseBytesWellDefined(
              replaceAt(Value), 0, Width
          )
          andBool Width ==Int lengthBytes(Value)
          andBool Width ==Int size(SB)
      [simplification]

endmodule

```