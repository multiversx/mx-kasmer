// LLVM backend implementation of the KRYPTO hooks declared in krypto.md.
//
// Compiled into the interpreter by passing this file to kompile with -ccopt.
// SHA-256 comes from OpenSSL. Keccak-256 (the original Keccak padding, not
// SHA3-256) is implemented here, because the OpenSSL version in the build
// image does not provide it.
//
// Tested end to end by mx-semantics/tests/mandos/crypto.scen.json.

#include <cstddef>
#include <cstdint>
#include <cstring>

#include <openssl/sha.h>

#include "runtime/alloc.h"
#include "runtime/header.h"

namespace {

const uint64_t KECCAK_ROUND_CONSTANTS[24] = {
    0x0000000000000001ULL, 0x0000000000008082ULL, 0x800000000000808aULL,
    0x8000000080008000ULL, 0x000000000000808bULL, 0x0000000080000001ULL,
    0x8000000080008081ULL, 0x8000000000008009ULL, 0x000000000000008aULL,
    0x0000000000000088ULL, 0x0000000080008009ULL, 0x000000008000000aULL,
    0x000000008000808bULL, 0x800000000000008bULL, 0x8000000000008089ULL,
    0x8000000000008003ULL, 0x8000000000008002ULL, 0x8000000000000080ULL,
    0x000000000000800aULL, 0x800000008000000aULL, 0x8000000080008081ULL,
    0x8000000000008080ULL, 0x0000000080000001ULL, 0x8000000080008008ULL};

const int KECCAK_ROTATION[24] = {1,  3,  6,  10, 15, 21, 28, 36, 45, 55, 2,  14,
                                 27, 41, 56, 8,  25, 43, 62, 18, 39, 61, 20, 44};

const int KECCAK_PI_LANE[24] = {10, 7,  11, 17, 18, 3, 5,  16, 8,  21, 24, 4,
                                15, 23, 19, 13, 12, 2, 20, 14, 22, 9,  6,  1};

inline uint64_t rotl64(uint64_t x, int n) { return (x << n) | (x >> (64 - n)); }

void keccak_f1600(uint64_t state[25]) {
  for (int round = 0; round < 24; round++) {
    uint64_t c[5];

    // theta
    for (int x = 0; x < 5; x++) {
      c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
    }
    for (int x = 0; x < 5; x++) {
      uint64_t d = c[(x + 4) % 5] ^ rotl64(c[(x + 1) % 5], 1);
      for (int y = 0; y < 25; y += 5) {
        state[y + x] ^= d;
      }
    }

    // rho and pi
    uint64_t current = state[1];
    for (int i = 0; i < 24; i++) {
      int lane = KECCAK_PI_LANE[i];
      uint64_t next = state[lane];
      state[lane] = rotl64(current, KECCAK_ROTATION[i]);
      current = next;
    }

    // chi
    for (int y = 0; y < 25; y += 5) {
      for (int x = 0; x < 5; x++) {
        c[x] = state[y + x];
      }
      for (int x = 0; x < 5; x++) {
        state[y + x] ^= (~c[(x + 1) % 5]) & c[(x + 2) % 5];
      }
    }

    // iota
    state[0] ^= KECCAK_ROUND_CONSTANTS[round];
  }
}

// XORs one byte into the sponge state at the given byte position.
// Lanes are little-endian; written with shifts so that it does not depend on
// the byte order of the host.
inline void absorb_byte(uint64_t state[25], size_t position, uint8_t byte) {
  state[position / 8] ^= static_cast<uint64_t>(byte) << (8 * (position % 8));
}

void keccak256(const uint8_t *input, size_t length, uint8_t digest[32]) {
  const size_t rate = 136; // bytes; 1600 bits minus twice the 256-bit capacity
  uint64_t state[25] = {0};

  size_t position = 0;
  for (size_t i = 0; i < length; i++) {
    absorb_byte(state, position, input[i]);
    if (++position == rate) {
      keccak_f1600(state);
      position = 0;
    }
  }

  // Keccak padding: 0x01, zeros, 0x80 (both in the same byte if it is the last one).
  absorb_byte(state, position, 0x01);
  absorb_byte(state, rate - 1, 0x80);
  keccak_f1600(state);

  for (size_t i = 0; i < 32; i++) {
    digest[i] = static_cast<uint8_t>(state[i / 8] >> (8 * (i % 8)));
  }
}

string *hex_encode(const uint8_t *bytes, size_t length) {
  static const char DIGITS[] = "0123456789abcdef";
  size_t hex_length = 2 * length;
  auto *result = static_cast<string *>(kore_alloc_token(sizeof(string) + hex_length));
  init_with_len(result, hex_length);
  for (size_t i = 0; i < length; i++) {
    result->data[2 * i] = DIGITS[bytes[i] >> 4];
    result->data[2 * i + 1] = DIGITS[bytes[i] & 0x0f];
  }
  return result;
}

} // namespace

extern "C" {

string *hook_KRYPTO_keccak256(string *input) {
  uint8_t digest[32];
  keccak256(reinterpret_cast<const uint8_t *>(input->data), len(input), digest);
  return hex_encode(digest, sizeof(digest));
}

string *hook_KRYPTO_sha256(string *input) {
  uint8_t digest[SHA256_DIGEST_LENGTH];
  SHA256(reinterpret_cast<const unsigned char *>(input->data), len(input), digest);
  return hex_encode(digest, sizeof(digest));
}

} // extern "C"
