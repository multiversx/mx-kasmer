Cryptographic primitives
========================

The hash functions used by the VM hooks. `String` results are hex-encoded,
`Bytes` arguments are raw byte strings.

These are hooked functions. The Haskell backend implements the `KRYPTO` hooks
itself; for the LLVM backend they are implemented in `krypto.cpp`, which is
compiled into the interpreter.

Only what the semantics uses is declared here. This replaces the `krypto.md`
of [blockchain-k-plugin](https://github.com/runtimeverification/blockchain-k-plugin),
from which the declarations are taken unchanged.

```k
module KRYPTO
    imports BYTES-SYNTAX
    imports STRING-SYNTAX

    syntax String ::= Keccak256 ( Bytes )  [function, hook(KRYPTO.keccak256)]
                    | Sha256 ( Bytes )     [function, hook(KRYPTO.sha256)]
endmodule
```
