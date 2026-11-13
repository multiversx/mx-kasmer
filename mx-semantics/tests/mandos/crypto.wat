(module
    (import "env" "mBufferNew" (func $mBufferNew (result i32)))
    (import "env" "mBufferGetArgument" (func $mBufferGetArgument (param i32 i32) (result i32)))
    (import "env" "mBufferFinish" (func $mBufferFinish (param i32) (result i32)))
    (import "env" "managedSha256" (func $managedSha256 (param i32 i32) (result i32)))
    (import "env" "managedKeccak256" (func $managedKeccak256 (param i32 i32) (result i32)))
    (memory 1)
    (func (export "init"))
    ;; returns sha256(argument 0)
    (func (export "sha256") (local $in i32) (local $out i32)
        call $mBufferNew
        local.set $in
        call $mBufferNew
        local.set $out
        i32.const 0
        local.get $in
        call $mBufferGetArgument
        drop
        local.get $in
        local.get $out
        call $managedSha256
        drop
        local.get $out
        call $mBufferFinish
        drop
    )
    ;; returns keccak256(argument 0)
    (func (export "keccak256") (local $in i32) (local $out i32)
        call $mBufferNew
        local.set $in
        call $mBufferNew
        local.set $out
        i32.const 0
        local.get $in
        call $mBufferGetArgument
        drop
        local.get $in
        local.get $out
        call $managedKeccak256
        drop
        local.get $out
        call $mBufferFinish
        drop
    )
)
