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
endmodule
