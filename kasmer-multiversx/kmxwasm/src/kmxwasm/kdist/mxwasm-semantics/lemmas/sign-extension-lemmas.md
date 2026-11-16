Sign-extension lemmas
=====================

The Wasm sign-extension operators (`i32.extend8_s`, `i32.extend16_s`,
`i64.extend8_s`, `i64.extend16_s`, `i64.extend32_s`) are emitted by default by
Rust compilers from version 1.82 on. The Wasm semantics evaluates them with

```
#extends(ITYPE, WIDTH, VAL) => < ITYPE > #unsigned(ITYPE, #signed(WIDTH, VAL))
  requires #numBytes(WIDTH) <=Int #numBytes(ITYPE) andBool definedSigned(WIDTH, VAL)
#extends(_, _, _) => undefined  [owise]
```

which only fires on concrete arguments. On a symbolic argument the term is left
as it is, the backend cannot rule out the `undefined` case, and execution ends
in a spurious `trap`. The lemmas below evaluate `#extends` symbolically when
the argument is known to fit in the signed range of the source width, which is
the common case: sign-extending the `i8` result of a comparison. A second lemma
covers an arbitrary argument.

```k
requires "../ceils-syntax.k"
requires "wasm-semantics/wasm.md"

module SIGN-EXTENSION-LEMMAS  [symbolic]
  imports private BOOL
  imports private CEILS-SYNTAX
  imports private INT
  imports private WASM
```

`#wrap(N, X)` is `X` modulo `2 ^ (8 * N)`, so a previous reduction modulo a
multiple of that power of two does not change the result.

```k
  rule #wrap(N:Int, X:Int modIntTotal M:Int) => #wrap(N, X)
      requires 0 <Int N
       andBool 0 <Int M
       andBool M modInt (1 <<Int (N *Int 8)) ==Int 0
      [simplification, concrete(N, M), preserves-definedness]
      // M is positive and so is the shifted value, no division by zero
```

Let `w` be the bit width of `W` and `t` the bit width of `TO`, with `w <= t`,
and assume `-2^(w-1) <= X < 2^(w-1)`. Then:

* `#wrap(N, X) = X mod 2^w`, which is in `[0, 2^w)`, so `definedSigned` holds;
* `#signed(W, X mod 2^w) = X`, because `X` is in the signed range of `W`;
* `#unsigned(TO, X)` is `X` when `0 <= X` and `X + 2^t` otherwise, which is
  `X mod 2^t` since `|X| <= 2^(w-1) < 2^t`.

```k
  rule #extends(TO:IValType, W:IWidth, #wrap(N:Int, X:Int))
        => < TO > (X modIntTotal #pow(TO))
      requires N ==Int #numBytes(W)
       andBool #numBytes(W) <=Int #numBytes(TO)
       andBool 0 -Int #pow1(W) <=Int X
       andBool X <Int #pow1(W)
      [simplification, concrete(TO, W, N)]
```

For an arbitrary `X`, let `S = ((X + 2^(w-1)) mod 2^w) - 2^(w-1)`. `S` is in the
signed range of `W` and congruent to `X` modulo `2^w`, so it is the value that
`#signed(W, #wrap(N, X))` computes. The result is `S` modulo `2^t`.
This lemma has a lower priority than the previous one, which gives a simpler
term when its side condition can be proven.

```k
  rule #extends(TO:IValType, W:IWidth, #wrap(N:Int, X:Int))
        => < TO > ((((X +Int #pow1(W)) modIntTotal #pow(W)) -Int #pow1(W)) modIntTotal #pow(TO))
      requires N ==Int #numBytes(W)
       andBool #numBytes(W) <=Int #numBytes(TO)
      [simplification(60), concrete(TO, W, N)]

endmodule
```
