Equality lemmas for the booster backend
=======================================

`lemmas/proven-mx-lemmas.md` states some simplifications over the matching-logic
equality `{ _ #Equals _ }`. The booster backend also meets the same facts as
the Boolean function `_==Int_`, and does not use one form for the other. The
rules below repeat three of those lemmas in the `==Int` form.

They were added upstream directly to the generated file
(runtimeverification/kasmer-multiversx#172). They live here instead, so that
regenerating `proven-mx-lemmas.md` reproduces the checked-in file.

Unlike the rules in `proven-mx-lemmas.md`, these are not machine-checked. Each
is the `==Int` counterpart of a proven rule there:

* `#bool(B)` is `1` when `B` holds and `0` otherwise, which gives the first two.
* For the third, with `M =/=Int 0` both sides say that `Y` and `Z` are congruent
  modulo `M`. With `M ==Int 0`, `_ modIntTotal 0` is the same value on both
  sides of each equality, so both sides are `true`.

```k
requires "../ceils-syntax.k"
requires "mx-semantics/elrond.md"

module BOOSTER-EQUALITY-LEMMAS  [symbolic]
    imports private BOOL
    imports private CEILS-SYNTAX
    imports private ELROND
    imports private INT

    rule  0 ==Int #bool ( B:Bool )  => notBool B
      [simplification()]

    rule  1 ==Int #bool ( B:Bool )  => B:Bool
      [simplification()]

    rule  (X +Int Y) modIntTotal M ==Int (X +Int Z) modIntTotal M
          => Y modIntTotal M ==Int Z modIntTotal M
      [simplification()]

endmodule
```
