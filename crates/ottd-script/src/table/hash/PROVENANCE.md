CityHash algorithm: Google, MIT; see CITYHASH-LICENSE. Version 1.0.3 source:
https://github.com/google/cityhash/blob/00b9287e8c1255b5922ef90e304d5287361b2c2a/src/city.cc
(the full verified commit is recorded in the evidence source manifest).

The independent safe Rust implementation follows this algorithm with the native
libc++ ABI-1 quirks observed in Apple __functional/hash.h: truncate the 4..8-byte
first-word shift to 32 bits, and add rather than replace z in the 33..64-byte path.
C++ library headers define the compatibility target, not a production dependency.

MurmurHash64A: Austin Appleby, public domain, copyright disclaimed:
https://github.com/aappleby/smhasher/blob/07bb4de10a63e8cc2e1724865454eba635742383/src/MurmurHash2.cpp
The Rust implementation uses native libstdc++'s seed 0xc70f6907.
No libstdc++ implementation text is incorporated.

Only 64-bit little-endian Apple libc++ ABI 1 and Linux GNU libstdc++ are admitted.
Native runtime probes must match the selected algorithm on the executing target.
