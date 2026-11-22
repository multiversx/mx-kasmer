module SIGN-EXTENSION
    imports MX-WASM-LEMMA-TESTS

    // Argument within the signed range of the source width: the result is the argument modulo 2^t
    claim
      <k>
        runSimplification(
            ListItem( #extends(i32, i8, #wrap(1, X:Int)) )
            ListItem( #extends(i64, i32, #wrap(4, X:Int)) )
        ) => doneSimplification(
            ListItem( < i32 > (X modIntTotal 4294967296) )
            ListItem( < i64 > (X modIntTotal 18446744073709551616) )
        )
      </k>
      <commands> .K </commands>
      <instrs> .K </instrs>
      requires 0 -Int 128 <=Int X andBool X <Int 128

    // Arbitrary argument
    claim
      <k>
        runSimplification(
            ListItem( #extends(i32, i8, #wrap(1, X:Int)) )
            ListItem( #extends(i32, i16, #wrap(2, X:Int)) )
            ListItem( #extends(i64, i32, #wrap(4, X:Int)) )
        ) => doneSimplification(
            ListItem( < i32 > ((((X +Int 128) modIntTotal 256) -Int 128) modIntTotal 4294967296) )
            ListItem( < i32 > ((((X +Int 32768) modIntTotal 65536) -Int 32768) modIntTotal 4294967296) )
            ListItem( < i64 > ((((X +Int 2147483648) modIntTotal 4294967296) -Int 2147483648) modIntTotal 18446744073709551616) )
        )
      </k>
      <commands> .K </commands>
      <instrs> .K </instrs>

    // MVP idiom (shl, shr_s), argument within the signed range
    claim
      <k>
        runSimplification(
            ListItem( #signedTotal(i32, int64encoding(X:Int, -1, -1, -1, -1, -1, -1, -1, 3)) >>IntTotal 24 )
            ListItem( #signedTotal(i32, int64encoding(X:Int, -1, -1, -1, -1, -1, -1, 3, 2)) >>IntTotal 16 )
        ) => doneSimplification(
            ListItem( X )
            ListItem( X )
        )
      </k>
      <commands> .K </commands>
      <instrs> .K </instrs>
      requires 0 -Int 128 <=Int X andBool X <Int 128

    // MVP idiom on the result of a big integer comparison, as compiled for wasm32v1-none:
    // the range follows from the bounds of #bool
    claim
      <k>
        runSimplification(
            ListItem( #signedTotal(i32, int64encoding(#bool(100 <Int X:Int) +Int ((-1) *Int #bool(X <Int 100)),
                                                      -1, -1, -1, -1, -1, -1, -1, 3)) >>IntTotal 24 )
        ) => doneSimplification(
            ListItem( #bool(100 <Int X:Int) +Int ((-1) *Int #bool(X <Int 100)) )
        )
      </k>
      <commands> .K </commands>
      <instrs> .K </instrs>

    // ... and the comparison that follows it (i32.shr_s wraps its result in #unsigned)
    claim
      <k>
        runSimplification(
            ListItem( #signed(i32, #unsigned(i32, #bool(100 <Int X:Int) +Int ((-1) *Int #bool(X <Int 100)))) <Int 1 )
        ) => doneSimplification(
            ListItem( #bool(100 <Int X:Int) +Int ((-1) *Int #bool(X <Int 100)) <Int 1 )
        )
      </k>
      <commands> .K </commands>
      <instrs> .K </instrs>

    // MVP idiom, arbitrary argument
    claim
      <k>
        runSimplification(
            ListItem( #signedTotal(i32, int64encoding(X:Int, -1, -1, -1, -1, -1, -1, -1, 3)) >>IntTotal 24 )
            ListItem( #signedTotal(i32, int64encoding(X:Int, -1, -1, -1, -1, -1, -1, 3, 2)) >>IntTotal 16 )
        ) => doneSimplification(
            ListItem( ((X +Int 128) modIntTotal 256) -Int 128 )
            ListItem( ((X +Int 32768) modIntTotal 65536) -Int 32768 )
        )
      </k>
      <commands> .K </commands>
      <instrs> .K </instrs>
endmodule
