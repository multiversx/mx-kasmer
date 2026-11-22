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
requires "int-encoding-lemmas.md"
requires "wasm-semantics/wasm.md"

module SIGN-EXTENSION-LEMMAS  [symbolic]
  imports private BOOL
  imports private CEILS-SYNTAX
  imports private INT
  imports private INT-ENCODING-BASIC
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
```

Sign extension without the sign-extension operators
---------------------------------------------------

Targets without the sign-extension feature, such as `wasm32v1-none`, which
`sc-meta` builds for, sign-extend an `i8` to `i32` with the MVP idiom
`i32.const 24; i32.shl; i32.const 24; i32.shr_s`, and an `i16` with 16
instead of 24. After the integer encoding lemmas, `x << 24` truncated to 32
bits is `int64encoding(x, ..., b1pos: 3)`, byte `0` of `x` on position `3`, and
the arithmetic shift right is

```
#unsigned(i32, #signedTotal(i32, int64encoding(x, ..., b1pos: 3)) >>IntTotal 24)
```

Let `b = x mod 256`. The encoding is `b * 2^24`, in `[0, 2^32)`. Its signed
value is `b * 2^24` when `b < 128` and `b * 2^24 - 2^32` otherwise, and
shifting right by 24 (rounding down) gives `b` or `b - 256`, which is
`((x + 128) mod 256) - 128`. That is `x` itself when `x` is in the signed range
of `i8`. The `i16` case is the same with bytes `0` and `1` on positions `2` and
`3`, `2^16` for `256` and `2^15` for `128`. As above, the general lemma has a
lower priority.

```k
  rule #signedTotal(i32, int64encoding(X:Int, -1, -1, -1, -1, -1, -1, -1, 3)) >>IntTotal 24 => X
      requires 0 -Int 128 <=Int X andBool X <Int 128
      [simplification]

  rule #signedTotal(i32, int64encoding(X:Int, -1, -1, -1, -1, -1, -1, -1, 3)) >>IntTotal 24
        => ((X +Int 128) modIntTotal 256) -Int 128
      [simplification(60)]

  rule #signedTotal(i32, int64encoding(X:Int, -1, -1, -1, -1, -1, -1, 3, 2)) >>IntTotal 16 => X
      requires 0 -Int 32768 <=Int X andBool X <Int 32768
      [simplification]

  rule #signedTotal(i32, int64encoding(X:Int, -1, -1, -1, -1, -1, -1, 3, 2)) >>IntTotal 16
        => ((X +Int 32768) modIntTotal 65536) -Int 32768
      [simplification(60)]
```

`i32.shr_s` wraps its result in `#unsigned(i32, _)`, which is defined by two
rules on the sign of the argument and stays unevaluated on a symbolic argument
of unknown sign. The comparison that follows (`lt_s`) then cannot rule out an
undefined `#signed` and execution ends in a spurious `trap`. For
`-2^t <= X < 2^t`, `#unsigned(T, X)` is `X mod 2^t` in both cases; in that form
the lemmas for `#signed` of a `modIntTotal` apply, as they do for the result of
`#extends` above.

```k
  rule #unsigned(T:IValType, X:Int) => X modIntTotal #pow(T)
      requires 0 -Int #pow(T) <=Int X andBool X <Int #pow(T)
      [simplification, symbolic(X), concrete(T), preserves-definedness]
      // #pow(T) is positive, no division by zero

endmodule
```
